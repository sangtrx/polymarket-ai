"use client";

import { useEffect, useState } from "react";
import { RiskPostureBanner } from "@/components/risk/RiskPostureBanner";
import { SafetyActionRail } from "@/components/risk/SafetyActionRail";
import { getOperatorConsoleEnv } from "@/lib/env";
import {
  type RiskPostureViewModel,
  withEmergencyControlEvidence,
} from "@/lib/risk/posture";

export function RiskCommandSurface({
  riskPosture,
}: {
  riskPosture: RiskPostureViewModel;
}) {
  const [activePosture, setActivePosture] = useState(riskPosture);
  const { apiBaseUrl } = getOperatorConsoleEnv();

  useEffect(() => {
    setActivePosture(riskPosture);
  }, [riskPosture]);

  return (
    <section
      aria-label="Risk posture and persistent safety controls"
      className="risk-command-surface"
    >
      <RiskPostureBanner model={activePosture} />
      <SafetyActionRail
        baseUrl={apiBaseUrl}
        onActionConfirmed={(decision) => {
          setActivePosture((previous) =>
            withEmergencyControlEvidence(previous, decision),
          );
        }}
        resumeState={activePosture.resumeState}
        resumeStateReason={activePosture.resumeStateReason}
      />
    </section>
  );
}
