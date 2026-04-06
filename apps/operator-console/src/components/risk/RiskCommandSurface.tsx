"use client";

import { useEffect, useState } from "react";
import { RiskPostureBanner } from "@/components/risk/RiskPostureBanner";
import { SafetyActionRail } from "@/components/risk/SafetyActionRail";
import { getOperatorConsoleEnv } from "@/lib/env";
import {
  type RiskPostureViewModel,
  withRecoveryReadinessEvidence,
  withRecoveryResumeEvidence,
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
        onRecoveryEvaluated={(decision) => {
          setActivePosture((previous) =>
            withRecoveryReadinessEvidence(previous, decision),
          );
        }}
        onRecoveryResumed={(decision) => {
          setActivePosture((previous) =>
            withRecoveryResumeEvidence(previous, decision),
          );
        }}
        resumeState={activePosture.resumeState}
        resumeStateReason={activePosture.resumeStateReason}
        resumeFailureDetails={activePosture.resumeFailureDetails}
        resumeProfileKey={activePosture.resumeProfileKey}
        resumeReconciliationRunId={activePosture.resumeReconciliationRunId}
        resumeApprovedChecksum={activePosture.resumeApprovedChecksum}
        resumeSignoffIntent={activePosture.resumeSignoffIntent}
        resumeArtifactId={activePosture.resumeArtifactId}
        resumeIncidentCorrelationId={activePosture.resumeIncidentCorrelationId}
        resumeIncidentSeverity={activePosture.resumeIncidentSeverity}
        resumeRehearsalRunId={activePosture.resumeRehearsalRunId}
      />
    </section>
  );
}
