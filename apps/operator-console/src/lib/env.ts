const LOCAL_CONTROL_API_URL = "http://127.0.0.1:8080";

function resolveDefaultControlApiUrl(): string {
  return process.env.NODE_ENV === "development" ? LOCAL_CONTROL_API_URL : "";
}

export function getOperatorConsoleEnv() {
  return {
    apiBaseUrl:
      process.env.NEXT_PUBLIC_OPERATOR_CONSOLE_API_BASE_URL ??
      process.env.OPERATOR_CONSOLE_PUBLIC_API_BASE_URL ??
      resolveDefaultControlApiUrl(),
  };
}
