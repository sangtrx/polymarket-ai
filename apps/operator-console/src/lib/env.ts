const LOCAL_CONTROL_API_URL = "http://127.0.0.1:8080";
const LOCAL_OPERATOR_BFF_URL = "http://127.0.0.1:8090";

function resolveDefaultControlApiUrl(): string {
  return process.env.NODE_ENV === "development" ? LOCAL_CONTROL_API_URL : "";
}

function resolveDefaultOperatorBffUrl(): string {
  return process.env.NODE_ENV === "development" ? LOCAL_OPERATOR_BFF_URL : "";
}

export function getOperatorConsoleEnv() {
  return {
    apiBaseUrl:
      process.env.NEXT_PUBLIC_OPERATOR_CONSOLE_API_BASE_URL ??
      process.env.OPERATOR_CONSOLE_PUBLIC_API_BASE_URL ??
      resolveDefaultControlApiUrl(),
    operatorBffBaseUrl:
      process.env.OPERATOR_BFF_INTERNAL_BASE_URL ??
      process.env.OPERATOR_BFF_PUBLIC_BASE_URL ??
      process.env.NEXT_PUBLIC_OPERATOR_BFF_BASE_URL ??
      resolveDefaultOperatorBffUrl(),
  };
}
