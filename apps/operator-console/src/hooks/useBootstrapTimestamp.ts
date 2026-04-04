"use client";

import { useMemo } from "react";

export function useBootstrapTimestamp() {
  return useMemo(() => new Date().toISOString(), []);
}
