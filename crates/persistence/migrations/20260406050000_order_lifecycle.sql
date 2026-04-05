CREATE TABLE IF NOT EXISTS orders (
    order_id TEXT PRIMARY KEY,
    market_id TEXT NOT NULL,
    order_mode TEXT NOT NULL,
    lifecycle_state TEXT NOT NULL,
    submission_idempotency_key TEXT NOT NULL,
    cancel_idempotency_key TEXT,
    last_transition_sequence BIGINT NOT NULL,
    last_reason_code TEXT NOT NULL,
    correlation_id TEXT NOT NULL,
    created_at_utc TIMESTAMPTZ NOT NULL,
    updated_at_utc TIMESTAMPTZ NOT NULL,
    recorded_at_utc TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (char_length(trim(order_id)) > 0),
    CHECK (char_length(trim(market_id)) > 0),
    CHECK (order_mode IN ('limit', 'reduce_only')),
    CHECK (
        lifecycle_state IN (
            'pending',
            'live',
            'partially_filled',
            'filled',
            'canceled',
            'expired'
        )
    ),
    CHECK (
        last_reason_code IN (
            'order_lifecycle_submission_accepted',
            'order_lifecycle_cancel_accepted',
            'order_lifecycle_batch_cancel_accepted',
            'order_lifecycle_venue_update_accepted',
            'order_lifecycle_transition_rejected',
            'order_lifecycle_unsupported_venue_state',
            'order_lifecycle_already_terminal',
            'order_lifecycle_duplicate_idempotency_key',
            'order_lifecycle_retryable_failure',
            'order_lifecycle_hard_failure',
            'order_lifecycle_persistence_unavailable',
            'order_lifecycle_venue_unavailable',
            'order_lifecycle_unauthorized',
            'order_lifecycle_auth_expired',
            'order_lifecycle_invalid_payload'
        )
    ),
    CHECK (char_length(trim(submission_idempotency_key)) > 0),
    CHECK (submission_idempotency_key = lower(trim(submission_idempotency_key))),
    CHECK (
        cancel_idempotency_key IS NULL
        OR (
            char_length(trim(cancel_idempotency_key)) > 0
            AND cancel_idempotency_key = lower(trim(cancel_idempotency_key))
        )
    ),
    CHECK (char_length(trim(correlation_id)) > 0),
    CHECK (last_transition_sequence > 0),
    CHECK (EXTRACT(TIMEZONE FROM created_at_utc) = 0),
    CHECK (EXTRACT(TIMEZONE FROM updated_at_utc) = 0),
    CHECK (EXTRACT(TIMEZONE FROM recorded_at_utc) = 0),
    CHECK (updated_at_utc >= created_at_utc)
);

CREATE INDEX IF NOT EXISTS idx_orders_market_state_updated
    ON orders (market_id, lifecycle_state, updated_at_utc DESC);

CREATE INDEX IF NOT EXISTS idx_orders_updated
    ON orders (updated_at_utc DESC);

CREATE TABLE IF NOT EXISTS order_state_transitions (
    transition_id TEXT PRIMARY KEY,
    order_id TEXT NOT NULL REFERENCES orders(order_id) ON DELETE CASCADE,
    market_id TEXT NOT NULL,
    order_mode TEXT NOT NULL,
    from_state TEXT,
    to_state TEXT NOT NULL,
    reason_code TEXT NOT NULL,
    idempotency_key TEXT NOT NULL,
    correlation_id TEXT NOT NULL,
    transition_sequence BIGINT NOT NULL,
    transitioned_at_utc TIMESTAMPTZ NOT NULL,
    recorded_at_utc TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (char_length(trim(transition_id)) > 0),
    CHECK (char_length(trim(order_id)) > 0),
    CHECK (char_length(trim(market_id)) > 0),
    CHECK (order_mode IN ('limit', 'reduce_only')),
    CHECK (
        from_state IS NULL
        OR from_state IN (
            'pending',
            'live',
            'partially_filled',
            'filled',
            'canceled',
            'expired'
        )
    ),
    CHECK (
        to_state IN (
            'pending',
            'live',
            'partially_filled',
            'filled',
            'canceled',
            'expired'
        )
    ),
    CHECK (
        reason_code IN (
            'order_lifecycle_submission_accepted',
            'order_lifecycle_cancel_accepted',
            'order_lifecycle_batch_cancel_accepted',
            'order_lifecycle_venue_update_accepted',
            'order_lifecycle_transition_rejected',
            'order_lifecycle_unsupported_venue_state',
            'order_lifecycle_already_terminal',
            'order_lifecycle_duplicate_idempotency_key',
            'order_lifecycle_retryable_failure',
            'order_lifecycle_hard_failure',
            'order_lifecycle_persistence_unavailable',
            'order_lifecycle_venue_unavailable',
            'order_lifecycle_unauthorized',
            'order_lifecycle_auth_expired',
            'order_lifecycle_invalid_payload'
        )
    ),
    CHECK (char_length(trim(reason_code)) > 0),
    CHECK (char_length(trim(idempotency_key)) > 0),
    CHECK (idempotency_key = lower(trim(idempotency_key))),
    CHECK (char_length(trim(correlation_id)) > 0),
    CHECK (transition_sequence > 0),
    CHECK (EXTRACT(TIMEZONE FROM transitioned_at_utc) = 0),
    CHECK (EXTRACT(TIMEZONE FROM recorded_at_utc) = 0),
    UNIQUE (order_id, transition_sequence),
    UNIQUE (order_id, idempotency_key)
);

CREATE INDEX IF NOT EXISTS idx_order_state_transitions_order_replay
    ON order_state_transitions (order_id, transition_sequence ASC);

CREATE INDEX IF NOT EXISTS idx_order_state_transitions_order_time
    ON order_state_transitions (order_id, transitioned_at_utc DESC);

CREATE INDEX IF NOT EXISTS idx_order_state_transitions_correlation_time
    ON order_state_transitions (correlation_id, transitioned_at_utc DESC);

CREATE INDEX IF NOT EXISTS idx_order_state_transitions_market_time
    ON order_state_transitions (market_id, transitioned_at_utc DESC);

CREATE OR REPLACE FUNCTION enforce_order_transition_sequence_monotonicity()
RETURNS trigger AS $$
DECLARE
    latest_sequence BIGINT;
BEGIN
    SELECT MAX(transition_sequence)
    INTO latest_sequence
    FROM order_state_transitions
    WHERE order_id = NEW.order_id;

    IF latest_sequence IS NULL THEN
        IF NEW.transition_sequence <> 1 THEN
            RAISE EXCEPTION
                'first order transition sequence must equal 1 for order_id % (got %)',
                NEW.order_id,
                NEW.transition_sequence;
        END IF;
        IF NEW.from_state IS NOT NULL THEN
            RAISE EXCEPTION
                'first order transition must have from_state = NULL for order_id %',
                NEW.order_id;
        END IF;
        RETURN NEW;
    END IF;

    IF NEW.transition_sequence <= latest_sequence THEN
        RAISE EXCEPTION
            'order_state_transitions.transition_sequence must be strictly increasing for order_id % (latest %, new %)',
            NEW.order_id,
            latest_sequence,
            NEW.transition_sequence;
    END IF;

    IF NEW.from_state IS NULL THEN
        RAISE EXCEPTION
            'non-initial order transition requires non-null from_state for order_id %',
            NEW.order_id;
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_order_state_transitions_sequence_monotonicity ON order_state_transitions;

CREATE TRIGGER trg_order_state_transitions_sequence_monotonicity
BEFORE INSERT ON order_state_transitions
FOR EACH ROW
EXECUTE FUNCTION enforce_order_transition_sequence_monotonicity();
