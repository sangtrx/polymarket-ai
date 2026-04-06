"use client";

import { usePathname, useRouter, useSearchParams } from "next/navigation";
import { type KeyboardEvent, useEffect, useMemo, useRef } from "react";

export interface InPageTabDefinition {
  id: string;
  label: string;
  content: React.ReactNode;
}

interface InPageTabsProps {
  tabs: readonly InPageTabDefinition[];
  queryKey?: string;
  ariaLabel?: string;
}

function normalizeTabs(
  tabs: readonly InPageTabDefinition[],
): InPageTabDefinition[] {
  const seen = new Set<string>();
  const normalized: InPageTabDefinition[] = [];

  for (const tab of tabs) {
    const id = tab.id.trim();
    if (!id || seen.has(id)) {
      continue;
    }

    seen.add(id);
    normalized.push({ ...tab, id });
  }

  return normalized;
}

function withQueryParam(
  searchParams: URLSearchParams,
  queryKey: string,
  value: string,
): string {
  const next = new URLSearchParams(searchParams.toString());
  next.set(queryKey, value);
  return next.toString();
}

export function InPageTabs({
  tabs,
  queryKey = "view",
  ariaLabel = "Workflow sub-views",
}: InPageTabsProps) {
  const router = useRouter();
  const pathname = usePathname();
  const searchParams = useSearchParams();
  const tabRefs = useRef<Array<HTMLButtonElement | null>>([]);
  const resolvedTabs = useMemo(() => normalizeTabs(tabs), [tabs]);

  const activeTabId = useMemo(() => {
    const current = searchParams.get(queryKey);
    if (current && resolvedTabs.some((tab) => tab.id === current)) {
      return current;
    }
    return resolvedTabs[0]?.id;
  }, [queryKey, resolvedTabs, searchParams]);

  useEffect(() => {
    const current = searchParams.get(queryKey);
    const fallbackId = resolvedTabs[0]?.id;

    if (
      !current ||
      !fallbackId ||
      resolvedTabs.some((tab) => tab.id === current)
    ) {
      return;
    }

    const nextQuery = withQueryParam(
      new URLSearchParams(searchParams.toString()),
      queryKey,
      fallbackId,
    );
    router.replace(`${pathname}?${nextQuery}`, { scroll: false });
  }, [pathname, queryKey, resolvedTabs, router, searchParams]);

  const activeIndex = useMemo(
    () => resolvedTabs.findIndex((tab) => tab.id === activeTabId),
    [activeTabId, resolvedTabs],
  );

  const selectTab = (tabId: string) => {
    if (!resolvedTabs.some((tab) => tab.id === tabId)) {
      return;
    }

    const nextQuery = withQueryParam(
      new URLSearchParams(searchParams.toString()),
      queryKey,
      tabId,
    );
    router.replace(`${pathname}?${nextQuery}`, { scroll: false });
  };

  const focusAndSelect = (index: number) => {
    const nextTab = resolvedTabs[index];
    if (!nextTab) {
      return;
    }

    tabRefs.current[index]?.focus();
    selectTab(nextTab.id);
  };

  const onTabKeyDown = (
    event: KeyboardEvent<HTMLButtonElement>,
    index: number,
  ) => {
    let nextIndex = index;

    if (event.key === "ArrowRight" || event.key === "ArrowDown") {
      nextIndex = (index + 1) % resolvedTabs.length;
    } else if (event.key === "ArrowLeft" || event.key === "ArrowUp") {
      nextIndex = (index - 1 + resolvedTabs.length) % resolvedTabs.length;
    } else if (event.key === "Home") {
      nextIndex = 0;
    } else if (event.key === "End") {
      nextIndex = resolvedTabs.length - 1;
    } else if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      const fallbackTabId = resolvedTabs[0]?.id;
      if (!fallbackTabId) {
        return;
      }

      selectTab(resolvedTabs[index]?.id ?? fallbackTabId);
      return;
    } else {
      return;
    }

    event.preventDefault();
    focusAndSelect(nextIndex);
  };

  if (resolvedTabs.length === 0) {
    return null;
  }

  return (
    <section className="shell-tabs">
      <div aria-label={ariaLabel} className="shell-tablist" role="tablist">
        {resolvedTabs.map((tab, index) => {
          const isActive = tab.id === activeTabId;
          const tabId = `tab-${tab.id}`;
          const panelId = `panel-${tab.id}`;

          return (
            <button
              aria-controls={panelId}
              aria-selected={isActive}
              className="shell-tab"
              id={tabId}
              key={tab.id}
              onClick={() => selectTab(tab.id)}
              onKeyDown={(event) => onTabKeyDown(event, index)}
              ref={(element) => {
                tabRefs.current[index] = element;
              }}
              role="tab"
              tabIndex={isActive ? 0 : -1}
              type="button"
            >
              {tab.label}
            </button>
          );
        })}
      </div>

      {resolvedTabs.map((tab, index) => {
        const isActive = index === activeIndex;
        const tabId = `tab-${tab.id}`;
        const panelId = `panel-${tab.id}`;

        return (
          <section
            aria-labelledby={tabId}
            className="shell-tabpanel"
            hidden={!isActive}
            id={panelId}
            key={tab.id}
            role="tabpanel"
            tabIndex={0}
          >
            {tab.content}
          </section>
        );
      })}
    </section>
  );
}
