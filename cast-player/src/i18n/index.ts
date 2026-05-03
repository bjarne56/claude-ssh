import en from "./locales/en.json";
import zhCN from "./locales/zh-CN.json";
import zhTW from "./locales/zh-TW.json";
import ja from "./locales/ja.json";
import ko from "./locales/ko.json";
import fr from "./locales/fr.json";
import de from "./locales/de.json";
import es from "./locales/es.json";
import it from "./locales/it.json";
import ptBR from "./locales/pt-BR.json";
import pt from "./locales/pt.json";
import ru from "./locales/ru.json";
import uk from "./locales/uk.json";
import pl from "./locales/pl.json";
import cs from "./locales/cs.json";
import hu from "./locales/hu.json";
import ro from "./locales/ro.json";
import nl from "./locales/nl.json";
import sv from "./locales/sv.json";
import nb from "./locales/nb.json";
import da from "./locales/da.json";
import fi from "./locales/fi.json";
import el from "./locales/el.json";
import ar from "./locales/ar.json";
import he from "./locales/he.json";
import tr from "./locales/tr.json";
import hi from "./locales/hi.json";
import id from "./locales/id.json";
import ms from "./locales/ms.json";
import fil from "./locales/fil.json";
import vi from "./locales/vi.json";
import th from "./locales/th.json";

type Translations = typeof zhCN;

// 深度 key path: "sidebar.title" → translations.sidebar.title
type DeepDotKey<T, P extends string = ""> = {
  [K in keyof T]: T[K] extends Record<string, unknown>
    ? DeepDotKey<T[K], `${P & string}${K & string}.`>
    : `${P & string}${K & string}`;
}[keyof T];

export type I18nKey = DeepDotKey<Translations>;

const locales: Record<string, Translations> = {
  en,
  "zh-CN": zhCN,
  "zh-TW": zhTW,
  ja,
  ko,
  fr,
  de,
  es,
  it,
  "pt-BR": ptBR,
  pt,
  ru,
  uk,
  pl,
  cs,
  hu,
  ro,
  nl,
  sv,
  nb,
  da,
  fi,
  el,
  ar,
  he,
  tr,
  hi,
  id,
  ms,
  fil,
  vi,
  th,
};

let currentLocale = "zh-CN";

export function setLocale(locale: string) {
  if (locales[locale]) {
    currentLocale = locale;
  } else {
    // 尝试语言前缀 fallback (zh-TW → zh, pt-BR → pt 已在 map 中)
  }
}

export function getLocale(): string {
  return currentLocale;
}

/** 自动检测系统 locale */
export function detectLocale(): string {
  if (typeof navigator !== "undefined" && navigator.language) {
    const lang = navigator.language;
    if (locales[lang]) return lang;
    // 尝试前缀
    const prefix = lang.split("-")[0];
    for (const key of Object.keys(locales)) {
      if (key.startsWith(prefix)) return key;
    }
  }
  return "zh-CN";
}

/**
 * t("sidebar.title") → "录像库"
 * t("commands.totalCount") → "总计"
 */
export function t(
  key: I18nKey,
  params?: Record<string, string | number>
): string {
  const parts = key.split(".");
  let value: unknown = locales[currentLocale];

  for (const part of parts) {
    if (value && typeof value === "object" && part in value) {
      value = (value as Record<string, unknown>)[part];
    } else {
      // fallback 到英文
      value = en;
      for (const p of parts) {
        if (value && typeof value === "object" && p in value) {
          value = (value as Record<string, unknown>)[p];
        } else {
          return key;
        }
      }
      break;
    }
  }

  let result = typeof value === "string" ? value : key;

  if (params) {
    for (const [k, v] of Object.entries(params)) {
      result = result.replace(`{{${k}}}`, String(v));
    }
  }

  return result;
}

import { useState, useCallback } from "react";

export function useTranslation() {
  const [, setTick] = useState(0);

  const translate = useCallback(
    (key: I18nKey, params?: Record<string, string | number>) => {
      return t(key, params);
    },
    []
  );

  const changeLocale = useCallback((locale: string) => {
    setLocale(locale);
    setTick((n) => n + 1);
  }, []);

  return { t: translate, locale: currentLocale, setLocale: changeLocale };
}

export default t;