pub fn telemetry_context_name() -> &'static str {
    std::any::type_name::<opentelemetry::Context>()
}
