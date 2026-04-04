const DEFAULT_CONTROL_API_URL = "http://127.0.0.1:8080";

export function getOperatorConsoleEnv() {
  return {
    apiBaseUrl:
      process.env.OPERATOR_CONSOLE_PUBLIC_API_BASE_URL ?? DEFAULT_CONTROL_API_URL,
  };
}
