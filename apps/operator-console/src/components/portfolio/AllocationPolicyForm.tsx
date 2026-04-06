"use client";

import { useMemo, useState } from "react";
import {
  AllocationPolicyClientError,
  type AllocationPolicyDecision,
  upsertAllocationPolicy,
} from "@/lib/portfolio/allocation-policy";

interface AllocationPolicyFormProps {
  baseUrl: string;
}

interface FormState {
  policyKey: string;
  portfolioScopeId: string;
  version: string;
  targetExposurePctNav: string;
  targetRelativeAlphaWeight: string;
  exposureDriftThresholdPct: string;
  relativeAlphaDriftThresholdPct: string;
  approvalRequestId: string;
  advancedParameters: string;
}

type FormField = keyof FormState;
type ValidationErrors = Partial<Record<FormField, string>>;
type GuidanceTone = "normal" | "warning" | "critical";

const DEFAULT_FORM_STATE: FormState = {
  policyKey: "portfolio-default",
  portfolioScopeId: "portfolio::default",
  version: "1",
  targetExposurePctNav: "30",
  targetRelativeAlphaWeight: "1.20",
  exposureDriftThresholdPct: "10",
  relativeAlphaDriftThresholdPct: "15",
  approvalRequestId: "",
  advancedParameters: "{}",
};

const POLICY_KEY_PATTERN = /^[a-z0-9._:-]{3,120}$/;

function toClientError(error: unknown): AllocationPolicyClientError {
  if (error instanceof AllocationPolicyClientError) {
    return error;
  }
  return new AllocationPolicyClientError({
    status: 500,
    errorCode: "allocation_policy_request_failed",
    message:
      error instanceof Error
        ? error.message
        : "Allocation policy request failed unexpectedly.",
    action: "allocation_policy_update",
    endpoint: "operator-console-ui",
    timestampUtc: new Date().toISOString(),
  });
}

function validateForm(values: FormState): ValidationErrors {
  const errors: ValidationErrors = {};

  if (!POLICY_KEY_PATTERN.test(values.policyKey.trim())) {
    errors.policyKey =
      "policy_key must be lowercase and contain 3-120 canonical characters.";
  }
  if (values.portfolioScopeId.trim().length < 3) {
    errors.portfolioScopeId = "portfolio_scope_id must not be blank.";
  }

  const version = Number(values.version);
  if (!Number.isInteger(version) || version <= 0) {
    errors.version = "version must be a positive integer.";
  }

  const targetExposure = Number(values.targetExposurePctNav);
  if (!Number.isFinite(targetExposure) || targetExposure < 0 || targetExposure > 100) {
    errors.targetExposurePctNav =
      "target_exposure_pct_nav must be finite within [0, 100].";
  }

  const targetAlpha = Number(values.targetRelativeAlphaWeight);
  if (!Number.isFinite(targetAlpha) || targetAlpha <= 0 || targetAlpha > 5) {
    errors.targetRelativeAlphaWeight =
      "target_relative_alpha_weight must be finite within (0, 5].";
  }

  const exposureThreshold = Number(values.exposureDriftThresholdPct);
  if (!Number.isFinite(exposureThreshold) || exposureThreshold <= 0 || exposureThreshold > 100) {
    errors.exposureDriftThresholdPct =
      "exposure_drift_threshold_pct must be finite within (0, 100].";
  }

  const alphaThreshold = Number(values.relativeAlphaDriftThresholdPct);
  if (!Number.isFinite(alphaThreshold) || alphaThreshold <= 0 || alphaThreshold > 100) {
    errors.relativeAlphaDriftThresholdPct =
      "relative_alpha_drift_threshold_pct must be finite within (0, 100].";
  }

  if (values.approvalRequestId.trim().length > 0 && values.approvalRequestId.trim().length < 4) {
    errors.approvalRequestId = "approval_request_id must be at least 4 characters when present.";
  }

  try {
    const parsed = JSON.parse(values.advancedParameters.trim() || "{}");
    if (Array.isArray(parsed) || typeof parsed !== "object" || parsed === null) {
      errors.advancedParameters = "advanced_parameters must be a JSON object.";
    }
  } catch {
    errors.advancedParameters = "advanced_parameters must be valid JSON.";
  }

  return errors;
}

function resolveGuidance(values: FormState): {
  tone: GuidanceTone;
  message: string;
  recommendedNextAction: string;
} {
  const exposureTarget = Number(values.targetExposurePctNav);
  const alphaWeight = Number(values.targetRelativeAlphaWeight);
  const exposureThreshold = Number(values.exposureDriftThresholdPct);
  const alphaThreshold = Number(values.relativeAlphaDriftThresholdPct);

  if (
    Number.isFinite(exposureTarget) &&
    Number.isFinite(alphaWeight) &&
    (exposureTarget >= 45 || alphaWeight >= 1.75)
  ) {
    return {
      tone: "critical",
      message:
        "Critical risk impact: target allocation materially increases concentration or alpha leverage assumptions.",
      recommendedNextAction:
        "Obtain dual approval evidence before promoting this policy to an execution-required path.",
    };
  }

  if (
    Number.isFinite(exposureThreshold) &&
    Number.isFinite(alphaThreshold) &&
    (exposureThreshold < 10 || alphaThreshold < 15)
  ) {
    return {
      tone: "warning",
      message:
        "Warning risk impact: tighter drift thresholds will increase recommendation frequency and operator workload.",
      recommendedNextAction:
        "Validate alert staffing and recommendation review cadence before lowering thresholds.",
    };
  }

  return {
    tone: "normal",
    message:
      "Normal risk impact: allocation targets and drift thresholds align with baseline policy defaults.",
    recommendedNextAction:
      "Submit policy and monitor recommendation telemetry for one control cycle.",
  };
}

export function AllocationPolicyForm({ baseUrl }: AllocationPolicyFormProps) {
  const [form, setForm] = useState<FormState>(DEFAULT_FORM_STATE);
  const [touched, setTouched] = useState<Partial<Record<FormField, boolean>>>({});
  const [errors, setErrors] = useState<ValidationErrors>({});
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [decision, setDecision] = useState<AllocationPolicyDecision | null>(null);
  const [requestError, setRequestError] = useState<AllocationPolicyClientError | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);

  const guidance = useMemo(() => resolveGuidance(form), [form]);

  const setField = (field: FormField, value: string) => {
    setForm((previous) => ({
      ...previous,
      [field]: value,
    }));
  };

  const markTouched = (field: FormField) => {
    setTouched((previous) => ({
      ...previous,
      [field]: true,
    }));
    setErrors(validateForm(form));
  };

  const visibleError = (field: FormField): string | undefined => {
    if (!touched[field]) {
      return undefined;
    }
    return errors[field];
  };

  const statusTone: GuidanceTone = requestError
    ? "critical"
    : decision?.approvalStatus === "pending"
      ? "warning"
      : "normal";

  return (
    <section className="allocation-policy-form" aria-label="Allocation policy form">
      <div className="shell-panel-header">
        <div>
          <p className="type-eyebrow">Allocation policy</p>
          <h3 className="type-heading-m">Configure policy + thresholds</h3>
        </div>
        <span className="shell-status-pill" data-tone={statusTone}>
          {requestError
            ? "error"
            : decision?.approvalStatus === "pending"
              ? "pending"
              : "ready"}
        </span>
      </div>

      <p className="type-body text-muted">
        Progressive disclosure keeps advanced controls optional while inline
        validation on blur enforces canonical payload safety.
      </p>

      <form
        className="allocation-form-grid"
        noValidate
        onSubmit={async (event) => {
          event.preventDefault();
          const validationErrors = validateForm(form);
          setErrors(validationErrors);
          setTouched({
            policyKey: true,
            portfolioScopeId: true,
            version: true,
            targetExposurePctNav: true,
            targetRelativeAlphaWeight: true,
            exposureDriftThresholdPct: true,
              relativeAlphaDriftThresholdPct: true,
              approvalRequestId: true,
              advancedParameters: true,
            });
          if (Object.keys(validationErrors).length > 0) {
            return;
          }

          if (!baseUrl.trim()) {
            setRequestError(
              new AllocationPolicyClientError({
                status: 500,
                errorCode: "allocation_policy_base_url_missing",
                message:
                  "Operator console API base URL is missing. Set NEXT_PUBLIC_OPERATOR_CONSOLE_API_BASE_URL.",
                action: "allocation_policy_update",
                endpoint: "operator-console-ui",
                timestampUtc: new Date().toISOString(),
              }),
            );
            return;
          }

          setIsSubmitting(true);
          setRequestError(null);
          try {
            const advancedParameters = JSON.parse(
              form.advancedParameters.trim() || "{}",
            ) as Record<string, unknown>;
            const nextDecision = await upsertAllocationPolicy({
              baseUrl,
              policyKey: form.policyKey.trim(),
              version: Number(form.version),
              portfolioScopeId: form.portfolioScopeId.trim(),
              targetExposurePctNav: Number(form.targetExposurePctNav),
              targetRelativeAlphaWeight: Number(form.targetRelativeAlphaWeight),
              exposureDriftThresholdPct: Number(form.exposureDriftThresholdPct),
              relativeAlphaDriftThresholdPct: Number(
                form.relativeAlphaDriftThresholdPct,
              ),
              advancedParameters,
              approvalRequestId: form.approvalRequestId.trim() || undefined,
            });
            setDecision(nextDecision);
          } catch (error) {
            setRequestError(toClientError(error));
          } finally {
            setIsSubmitting(false);
          }
        }}
      >
        <label className="allocation-form-field">
          <span className="type-metadata">Policy key</span>
          <input
            autoComplete="off"
            className="allocation-form-input"
            name="policy_key"
            onBlur={() => markTouched("policyKey")}
            onChange={(event) => setField("policyKey", event.target.value)}
            value={form.policyKey}
          />
          {visibleError("policyKey") ? (
            <p className="allocation-form-error">{visibleError("policyKey")}</p>
          ) : null}
        </label>

        <label className="allocation-form-field">
          <span className="type-metadata">Portfolio scope ID</span>
          <input
            autoComplete="off"
            className="allocation-form-input"
            name="portfolio_scope_id"
            onBlur={() => markTouched("portfolioScopeId")}
            onChange={(event) => setField("portfolioScopeId", event.target.value)}
            value={form.portfolioScopeId}
          />
          {visibleError("portfolioScopeId") ? (
            <p className="allocation-form-error">
              {visibleError("portfolioScopeId")}
            </p>
          ) : null}
        </label>

        <label className="allocation-form-field">
          <span className="type-metadata">Version</span>
          <input
            className="allocation-form-input"
            inputMode="numeric"
            name="version"
            onBlur={() => markTouched("version")}
            onChange={(event) => setField("version", event.target.value)}
            value={form.version}
          />
          {visibleError("version") ? (
            <p className="allocation-form-error">{visibleError("version")}</p>
          ) : null}
        </label>

        <label className="allocation-form-field">
          <span className="type-metadata">Target exposure (% NAV)</span>
          <input
            className="allocation-form-input"
            inputMode="decimal"
            name="target_exposure_pct_nav"
            onBlur={() => markTouched("targetExposurePctNav")}
            onChange={(event) => setField("targetExposurePctNav", event.target.value)}
            value={form.targetExposurePctNav}
          />
          {visibleError("targetExposurePctNav") ? (
            <p className="allocation-form-error">
              {visibleError("targetExposurePctNav")}
            </p>
          ) : null}
        </label>

        <label className="allocation-form-field">
          <span className="type-metadata">Target relative alpha weight</span>
          <input
            className="allocation-form-input"
            inputMode="decimal"
            name="target_relative_alpha_weight"
            onBlur={() => markTouched("targetRelativeAlphaWeight")}
            onChange={(event) =>
              setField("targetRelativeAlphaWeight", event.target.value)
            }
            value={form.targetRelativeAlphaWeight}
          />
          {visibleError("targetRelativeAlphaWeight") ? (
            <p className="allocation-form-error">
              {visibleError("targetRelativeAlphaWeight")}
            </p>
          ) : null}
        </label>

        <label className="allocation-form-field">
          <span className="type-metadata">Exposure drift threshold (%)</span>
          <input
            className="allocation-form-input"
            inputMode="decimal"
            name="exposure_drift_threshold_pct"
            onBlur={() => markTouched("exposureDriftThresholdPct")}
            onChange={(event) =>
              setField("exposureDriftThresholdPct", event.target.value)
            }
            value={form.exposureDriftThresholdPct}
          />
          {visibleError("exposureDriftThresholdPct") ? (
            <p className="allocation-form-error">
              {visibleError("exposureDriftThresholdPct")}
            </p>
          ) : null}
        </label>

        <label className="allocation-form-field">
          <span className="type-metadata">Relative alpha drift threshold (%)</span>
          <input
            className="allocation-form-input"
            inputMode="decimal"
            name="relative_alpha_drift_threshold_pct"
            onBlur={() => markTouched("relativeAlphaDriftThresholdPct")}
            onChange={(event) =>
              setField("relativeAlphaDriftThresholdPct", event.target.value)
            }
            value={form.relativeAlphaDriftThresholdPct}
          />
          {visibleError("relativeAlphaDriftThresholdPct") ? (
            <p className="allocation-form-error">
              {visibleError("relativeAlphaDriftThresholdPct")}
            </p>
          ) : null}
        </label>

        <label className="allocation-form-field">
          <span className="type-metadata">Approval request ID (optional)</span>
          <input
            autoComplete="off"
            className="allocation-form-input"
            name="approval_request_id"
            onBlur={() => markTouched("approvalRequestId")}
            onChange={(event) => setField("approvalRequestId", event.target.value)}
            value={form.approvalRequestId}
          />
          {visibleError("approvalRequestId") ? (
            <p className="allocation-form-error">
              {visibleError("approvalRequestId")}
            </p>
          ) : null}
        </label>

        <button
          aria-expanded={showAdvanced}
          className="allocation-advanced-toggle"
          onClick={() => setShowAdvanced((previous) => !previous)}
          type="button"
        >
          {showAdvanced ? "Hide advanced parameters" : "Show advanced parameters"}
        </button>

        {showAdvanced ? (
          <label className="allocation-form-field allocation-form-field--wide">
            <span className="type-metadata">Advanced parameters (JSON object)</span>
            <textarea
              className="allocation-form-textarea"
              name="advanced_parameters"
              onBlur={() => markTouched("advancedParameters")}
              onChange={(event) => setField("advancedParameters", event.target.value)}
              rows={4}
              value={form.advancedParameters}
            />
            {visibleError("advancedParameters") ? (
              <p className="allocation-form-error">
                {visibleError("advancedParameters")}
              </p>
            ) : null}
          </label>
        ) : null}

        <section className="allocation-guidance" data-tone={guidance.tone}>
          <p className="type-eyebrow">Risk-impact guidance</p>
          <p className="type-body text-muted">{guidance.message}</p>
          <p className="type-metadata">
            Recommended next action: {guidance.recommendedNextAction}
          </p>
        </section>

        <button className="allocation-form-submit" disabled={isSubmitting} type="submit">
          {isSubmitting ? "Submitting policy…" : "Submit allocation policy"}
        </button>
      </form>

      {decision ? (
        <section className="allocation-evidence" role="status">
          <p className="type-eyebrow">Policy mutation evidence</p>
          <dl className="risk-evidence-grid">
            <div>
              <dt className="type-metadata text-muted">Status</dt>
              <dd className="type-mono">{decision.status}</dd>
            </div>
            <div>
              <dt className="type-metadata text-muted">Approval context</dt>
              <dd className="type-mono">{decision.approvalStatus}</dd>
            </div>
            <div>
              <dt className="type-metadata text-muted">Reason code</dt>
              <dd className="type-mono">
                <code>{decision.reasonCode}</code>
              </dd>
            </div>
            <div>
              <dt className="type-metadata text-muted">Correlation ID</dt>
              <dd className="type-mono">{decision.correlationId}</dd>
            </div>
            <div>
              <dt className="type-metadata text-muted">Approval reference</dt>
              <dd className="type-mono">{decision.approvalReference ?? "n/a"}</dd>
            </div>
            <div>
              <dt className="type-metadata text-muted">Timestamp</dt>
              <dd className="type-mono">
                <time dateTime={decision.timestampUtc}>{decision.timestampUtc}</time>
              </dd>
            </div>
          </dl>
        </section>
      ) : null}

      {requestError ? (
        <section className="allocation-form-error-surface" role="alert">
          <p className="type-eyebrow">Allocation policy failure</p>
          <p className="type-body text-muted">{requestError.message}</p>
          <p className="type-metadata">
            Error code: <code>{requestError.errorCode}</code>
          </p>
          {requestError.fieldErrors.length > 0 ? (
            <ul className="allocation-form-error-list">
              {requestError.fieldErrors.map((issue) => (
                <li key={`${issue.field}:${issue.code}`} className="type-metadata">
                  <code>{issue.field}</code> — {issue.message} (<code>{issue.code}</code>)
                </li>
              ))}
            </ul>
          ) : null}
        </section>
      ) : null}
    </section>
  );
}
