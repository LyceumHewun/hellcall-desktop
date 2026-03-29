import { useEffect, useMemo } from "react";
import { RefreshCw, SatelliteDish } from "lucide-react";
import { toast } from "sonner";
import { useTranslation } from "react-i18next";
import { ImageWithFallback } from "../components/figma/ImageWithFallback";
import { KeySequence } from "../components/KeySequence";
import { StratagemCatalog } from "../../types/stratagems";
import { useStratagemsStore } from "../../store/stratagemsStore";

type SectionGroup = {
  section: string;
  categories: Array<{
    category: string;
    items: StratagemCatalog["items"];
  }>;
};

export function StratagemsView() {
  const { t } = useTranslation();
  const {
    catalog,
    isLoading,
    isRefreshing,
    hasLoaded,
    fetchCatalog,
    refreshCatalog,
  } = useStratagemsStore();

  useEffect(() => {
    fetchCatalog().catch((error) => {
      const message =
        error instanceof Error
          ? error.message
          : String(error ?? t("stratagems.update_failed"));
      toast.error(message);
    });
  }, [fetchCatalog, t]);

  const handleRefresh = async () => {
    try {
      await refreshCatalog();
      toast.success(t("stratagems.update_success"));
    } catch (error) {
      const message =
        error instanceof Error
          ? error.message
          : String(error ?? t("stratagems.update_failed"));
      toast.error(message);
    }
  };

  const groupedSections = useMemo<SectionGroup[]>(() => {
    if (!catalog?.items.length) {
      return [];
    }

    const sectionMap = new Map<string, Map<string, StratagemCatalog["items"]>>();

    for (const item of catalog.items) {
      const categoryMap =
        sectionMap.get(item.section) ?? new Map<string, StratagemCatalog["items"]>();
      const categoryItems = categoryMap.get(item.category) ?? [];
      categoryItems.push(item);
      categoryMap.set(item.category, categoryItems);
      sectionMap.set(item.section, categoryMap);
    }

    return Array.from(sectionMap.entries()).map(([section, categoryMap]) => ({
      section,
      categories: Array.from(categoryMap.entries()).map(([category, items]) => ({
        category,
        items,
      })),
    }));
  }, [catalog]);

  return (
    <>
      <div className="border-b border-white/10 p-6 shrink-0 bg-gradient-to-b from-[#0F1115] to-transparent backdrop-blur-sm">
        <div className="flex items-center justify-between">
          <div>
            <h1
              style={{ fontFamily: "var(--font-family-tech)" }}
              className="tracking-wider text-white mb-1 uppercase"
            >
              {t("stratagems.title")}
            </h1>
            <p className="text-white/50 text-sm">{t("stratagems.subtitle")}</p>
          </div>

          <button
            onClick={handleRefresh}
            disabled={isRefreshing}
            className="flex items-center gap-2 px-4 py-2.5 border-2 border-[#FCE100] text-[#FCE100] rounded hover:bg-[#FCE100]/10 transition-colors disabled:opacity-70 disabled:cursor-not-allowed"
          >
            <RefreshCw
              className={`w-4 h-4 ${isRefreshing ? "animate-spin" : ""}`}
            />
            <span>
              {isRefreshing
                ? t("stratagems.updating")
                : t("stratagems.update")}
            </span>
          </button>
        </div>
      </div>

      <div className="flex-1 overflow-y-auto p-6">
        <div className="max-w-6xl mx-auto space-y-6">
          {isLoading && !hasLoaded ? (
            <div className="rounded-xl border border-white/10 bg-[#1E2128] px-6 py-16 text-center text-white/55">
              {t("stratagems.loading")}
            </div>
          ) : groupedSections.length === 0 ? (
            <div className="rounded-xl border border-dashed border-white/15 bg-black/20 px-6 py-16 text-center">
              <p className="text-sm text-white/50">
                {t("stratagems.empty_hint")}
              </p>
            </div>
          ) : (
            groupedSections.map((section) => (
              <section key={section.section} className="space-y-4">
                <div className="flex items-center gap-3">
                  <div className="flex h-10 w-10 items-center justify-center rounded-full border border-[#FCE100]/30 bg-[#FCE100]/10 text-[#FCE100]">
                    <SatelliteDish className="w-5 h-5" />
                  </div>
                  <div>
                    <h2
                      style={{ fontFamily: "var(--font-family-tech)" }}
                      className="text-xl uppercase tracking-[0.18em] text-white"
                    >
                      {section.section}
                    </h2>
                    <p className="text-xs uppercase tracking-[0.18em] text-white/35">
                      {t("stratagems.category_count", {
                        count: section.categories.length,
                      })}
                    </p>
                  </div>
                </div>

                {section.categories.map((category) => (
                  <div
                    key={`${section.section}-${category.category}`}
                    className="overflow-hidden rounded-xl border border-white/10 bg-[#1E2128]"
                  >
                    <div className="border-b border-white/10 bg-black/20 px-5 py-3">
                      <div className="flex items-center justify-between gap-3">
                        <h3 className="text-sm font-semibold uppercase tracking-[0.18em] text-[#FCE100]">
                          {category.category}
                        </h3>
                        <span className="text-xs text-white/40">
                          {category.items.length} {t("stratagems.items")}
                        </span>
                      </div>
                    </div>

                    <div className="divide-y divide-white/10">
                      {category.items.map((item) => (
                        <div
                          key={`${item.category}-${item.name}`}
                          className="flex flex-col gap-4 px-5 py-4 md:flex-row md:items-center md:justify-between"
                        >
                          <div className="flex min-w-0 items-center gap-4">
                            <div className="flex h-14 w-14 shrink-0 items-center justify-center rounded-lg border border-white/10 bg-black/30 p-2">
                              <ImageWithFallback
                                src={item.icon_url}
                                alt={item.name}
                                className="h-10 w-10 object-contain"
                              />
                            </div>
                            <div className="min-w-0">
                              <p className="truncate text-base font-medium text-white">
                                {item.name}
                              </p>
                              <p className="text-xs uppercase tracking-[0.18em] text-white/40">
                                {item.category}
                              </p>
                            </div>
                          </div>

                          <div className="md:justify-end">
                            <KeySequence sequence={item.command} compact />
                          </div>
                        </div>
                      ))}
                    </div>
                  </div>
                ))}
              </section>
            ))
          )}
        </div>
      </div>
    </>
  );
}
