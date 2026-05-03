#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
同步: 给所有 29 国 locale (除 en/zh-CN) 补齐新增 keys。
新增 keys:
- app.language
- sidebar.notConfigured
- controls.speedLabel/skipIdleOn/skipIdleOff/jumpPlaceholder/
  progressClickHint/idleSegmentHint/markerCmd/markerDanger
- search.open
- export.open
- status.events/state

策略: 每个 locale 给一个明确翻译表; 已存在的 key 保留, 缺失的从 NEW 表填入。
"""
import json
import os

L = os.path.dirname(os.path.abspath(__file__)) + "/locales/"

# 完整的新增 keys (path → 各语言翻译)
NEW = {
    "app.language": {
        "zh-TW": "語言", "ja": "言語", "ko": "언어", "fr": "Langue", "de": "Sprache",
        "es": "Idioma", "it": "Lingua", "pt-BR": "Idioma", "pt": "Idioma",
        "ru": "Язык", "uk": "Мова", "pl": "Język", "cs": "Jazyk", "hu": "Nyelv",
        "ro": "Limbă", "nl": "Taal", "sv": "Språk", "nb": "Språk", "da": "Sprog",
        "fi": "Kieli", "el": "Γλώσσα", "ar": "اللغة", "he": "שפה", "tr": "Dil",
        "hi": "भाषा", "id": "Bahasa", "ms": "Bahasa", "fil": "Wika", "vi": "Ngôn ngữ", "th": "ภาษา",
    },
    "sidebar.notConfigured": {
        "zh-TW": "未設定", "ja": "未設定", "ko": "설정 안됨",
        "fr": "Non configuré", "de": "Nicht konfiguriert", "es": "No configurado",
        "it": "Non configurato", "pt-BR": "Não configurado", "pt": "Não configurado",
        "ru": "Не настроено", "uk": "Не налаштовано", "pl": "Nie skonfigurowane",
        "cs": "Nenakonfigurováno", "hu": "Nincs beállítva", "ro": "Neconfigurat",
        "nl": "Niet geconfigureerd", "sv": "Ej konfigurerad", "nb": "Ikke konfigurert",
        "da": "Ikke konfigureret", "fi": "Ei määritetty", "el": "Δεν έχει ρυθμιστεί",
        "ar": "غير مكوّن", "he": "לא מוגדר", "tr": "Yapılandırılmamış",
        "hi": "कॉन्फ़िगर नहीं", "id": "Belum dikonfigurasi", "ms": "Belum dikonfigurasi",
        "fil": "Hindi naka-configure", "vi": "Chưa cấu hình", "th": "ไม่ได้ตั้งค่า",
    },
    "controls.speedLabel": {
        "zh-TW": "倍速", "ja": "速度", "ko": "속도", "fr": "Vitesse", "de": "Geschw.",
        "es": "Vel.", "it": "Vel.", "pt-BR": "Vel.", "pt": "Vel.",
        "ru": "Скорость", "uk": "Швидкість", "pl": "Prędk.", "cs": "Rychl.",
        "hu": "Sebes.", "ro": "Vit.", "nl": "Snelh.", "sv": "Hast.",
        "nb": "Hast.", "da": "Hast.", "fi": "Nopeus", "el": "Ταχύτητα",
        "ar": "السرعة", "he": "מהירות", "tr": "Hız", "hi": "गति",
        "id": "Kecepatan", "ms": "Kelajuan", "fil": "Bilis", "vi": "Tốc độ", "th": "ความเร็ว",
    },
    "controls.skipIdleOn": {
        "zh-TW": "⚡ 跳過閒置", "ja": "⚡ アイドル省略", "ko": "⚡ 유휴 건너뜀",
        "fr": "⚡ Sauter inactif", "de": "⚡ Leerlauf überspr.",
        "es": "⚡ Saltar inactivo", "it": "⚡ Salta inattivo",
        "pt-BR": "⚡ Pular inativo", "pt": "⚡ Saltar inativo",
        "ru": "⚡ Пропуск", "uk": "⚡ Пропуск", "pl": "⚡ Pomiń bezczyn.",
        "cs": "⚡ Přesk. nečinnost", "hu": "⚡ Üresjárat ki", "ro": "⚡ Sari inactiv",
        "nl": "⚡ Inactief over", "sv": "⚡ Hoppa vilo", "nb": "⚡ Hopp over",
        "da": "⚡ Spring over", "fi": "⚡ Ohita joutoaika", "el": "⚡ Παράλειψη",
        "ar": "⚡ تخطي الخمول", "he": "⚡ דלג על מנוחה", "tr": "⚡ Boşta atla",
        "hi": "⚡ निष्क्रिय छोड़ें", "id": "⚡ Lewati diam", "ms": "⚡ Langkau melahu",
        "fil": "⚡ Laktawan idle", "vi": "⚡ Bỏ qua rảnh", "th": "⚡ ข้ามว่าง",
    },
    "controls.skipIdleOff": {
        "zh-TW": "⏳ 完整播放", "ja": "⏳ 完全再生", "ko": "⏳ 완전 재생",
        "fr": "⏳ Lecture complète", "de": "⏳ Volle Wiederg.",
        "es": "⏳ Repr. completa", "it": "⏳ Riproduz. completa",
        "pt-BR": "⏳ Reprod. completa", "pt": "⏳ Reprod. completa",
        "ru": "⏳ Полное воспр.", "uk": "⏳ Повне відтв.", "pl": "⏳ Pełne odtwarz.",
        "cs": "⏳ Úplné přehr.", "hu": "⏳ Teljes lej.", "ro": "⏳ Redare completă",
        "nl": "⏳ Volledige weerg.", "sv": "⏳ Full uppspeln.", "nb": "⏳ Full avspilling",
        "da": "⏳ Fuld afspiln.", "fi": "⏳ Täysi toisto", "el": "⏳ Πλήρης αναπ.",
        "ar": "⏳ تشغيل كامل", "he": "⏳ הפעלה מלאה", "tr": "⏳ Tam oynatma",
        "hi": "⏳ पूर्ण प्लेबैक", "id": "⏳ Putar penuh", "ms": "⏳ Main penuh",
        "fil": "⏳ Buong playback", "vi": "⏳ Phát đầy đủ", "th": "⏳ เล่นเต็ม",
    },
    "controls.jumpPlaceholder": {
        "zh-TW": "跳轉 (s)", "ja": "ジャンプ (秒)", "ko": "이동 (초)",
        "fr": "Aller à (s)", "de": "Springen (s)", "es": "Saltar a (s)",
        "it": "Vai a (s)", "pt-BR": "Ir para (s)", "pt": "Ir para (s)",
        "ru": "Перейти (с)", "uk": "Перейти (с)", "pl": "Skok (s)",
        "cs": "Skok (s)", "hu": "Ugrás (mp)", "ro": "Sari la (s)",
        "nl": "Spring (s)", "sv": "Hoppa (s)", "nb": "Hopp (s)",
        "da": "Hop (s)", "fi": "Hyppää (s)", "el": "Μετάβαση (s)",
        "ar": "اقفز (ث)", "he": "קפוץ (ש)", "tr": "Atla (s)",
        "hi": "जाएं (s)", "id": "Lompat (d)", "ms": "Lompat (s)",
        "fil": "Tumalon (s)", "vi": "Nhảy (s)", "th": "ข้าม (วิ)",
    },
    "controls.progressClickHint": {
        "zh-TW": "點擊跳轉", "ja": "クリックでシーク", "ko": "클릭하여 이동",
        "fr": "Cliquer pour aller à", "de": "Zum Springen klicken",
        "es": "Clic para buscar", "it": "Clicca per saltare",
        "pt-BR": "Clique para buscar", "pt": "Clique para procurar",
        "ru": "Нажмите для перехода", "uk": "Натисніть для переходу",
        "pl": "Kliknij, aby przejść", "cs": "Klikněte pro přesun",
        "hu": "Kattintson az ugráshoz", "ro": "Faceți clic pentru a căuta",
        "nl": "Klik om te zoeken", "sv": "Klicka för att hoppa",
        "nb": "Klikk for å hoppe", "da": "Klik for at hoppe",
        "fi": "Klikkaa siirtyäksesi", "el": "Κλικ για μετάβαση",
        "ar": "انقر للانتقال", "he": "לחץ כדי לעבור", "tr": "Atlamak için tıklayın",
        "hi": "जाने के लिए क्लिक करें", "id": "Klik untuk mencari",
        "ms": "Klik untuk pergi", "fil": "I-click upang lumipat",
        "vi": "Nhấp để chuyển", "th": "คลิกเพื่อข้าม",
    },
    "controls.idleSegmentHint": {
        "zh-TW": "閒置時段 (預設跳過)", "ja": "アイドル時間 (デフォルトでスキップ)",
        "ko": "유휴 시간 (기본 건너뜀)",
        "fr": "Période inactive (sautée par défaut)",
        "de": "Leerlaufzeit (standardmäßig übersprungen)",
        "es": "Período inactivo (omitido por defecto)",
        "it": "Periodo inattivo (saltato per default)",
        "pt-BR": "Período ocioso (pulado por padrão)",
        "pt": "Período inativo (saltado por padrão)",
        "ru": "Период бездействия (пропускается)",
        "uk": "Період бездіяльності (пропускається)",
        "pl": "Okres bezczynności (pomijany)",
        "cs": "Nečinné období (přeskočeno)",
        "hu": "Üresjárati időszak (átugorva)",
        "ro": "Perioadă inactivă (omisă implicit)",
        "nl": "Inactieve periode (overgeslagen)",
        "sv": "Vilotid (hoppas över som standard)",
        "nb": "Inaktiv periode (hoppes over)",
        "da": "Inaktiv periode (springes over)",
        "fi": "Joutoaika (ohitetaan oletuksena)",
        "el": "Αδρανής περίοδος (παράλειψη)",
        "ar": "فترة خمول (تتخطى افتراضياً)",
        "he": "תקופת מנוחה (מדולג כברירת מחדל)",
        "tr": "Boşta kalma (varsayılan atlanır)",
        "hi": "निष्क्रिय अवधि (डिफ़ॉल्ट छोड़ी)",
        "id": "Periode diam (dilewati default)",
        "ms": "Tempoh melahu (dilangkau)",
        "fil": "Idle period (default skipped)",
        "vi": "Thời gian rảnh (bỏ qua mặc định)",
        "th": "ช่วงว่าง (ข้ามค่าเริ่มต้น)",
    },
    "controls.markerCmd": {
        "zh-TW": "指令執行點", "ja": "コマンド", "ko": "명령어",
        "fr": "Commande", "de": "Befehl", "es": "Comando", "it": "Comando",
        "pt-BR": "Comando", "pt": "Comando", "ru": "Команда", "uk": "Команда",
        "pl": "Polecenie", "cs": "Příkaz", "hu": "Parancs", "ro": "Comandă",
        "nl": "Opdracht", "sv": "Kommando", "nb": "Kommando", "da": "Kommando",
        "fi": "Komento", "el": "Εντολή", "ar": "أمر", "he": "פקודה",
        "tr": "Komut", "hi": "कमांड", "id": "Perintah", "ms": "Arahan",
        "fil": "Utos", "vi": "Lệnh", "th": "คำสั่ง",
    },
    "controls.markerDanger": {
        "zh-TW": "危險指令", "ja": "危険コマンド", "ko": "위험한 명령어",
        "fr": "Commande dangereuse", "de": "Gefährlicher Befehl",
        "es": "Comando peligroso", "it": "Comando pericoloso",
        "pt-BR": "Comando perigoso", "pt": "Comando perigoso",
        "ru": "Опасная команда", "uk": "Небезпечна команда",
        "pl": "Niebezpieczne polecenie", "cs": "Nebezpečný příkaz",
        "hu": "Veszélyes parancs", "ro": "Comandă periculoasă",
        "nl": "Gevaarlijke opdracht", "sv": "Farligt kommando",
        "nb": "Farlig kommando", "da": "Farlig kommando",
        "fi": "Vaarallinen komento", "el": "Επικίνδυνη εντολή",
        "ar": "أمر خطير", "he": "פקודה מסוכנת", "tr": "Tehlikeli komut",
        "hi": "खतरनाक कमांड", "id": "Perintah berbahaya",
        "ms": "Arahan berbahaya", "fil": "Mapanganib na utos",
        "vi": "Lệnh nguy hiểm", "th": "คำสั่งอันตราย",
    },
    "search.open": {
        "zh-TW": "搜尋 (Ctrl+F)", "ja": "検索 (Ctrl+F)", "ko": "검색 (Ctrl+F)",
        "fr": "Rechercher (Ctrl+F)", "de": "Suchen (Strg+F)",
        "es": "Buscar (Ctrl+F)", "it": "Cerca (Ctrl+F)",
        "pt-BR": "Buscar (Ctrl+F)", "pt": "Pesquisar (Ctrl+F)",
        "ru": "Поиск (Ctrl+F)", "uk": "Пошук (Ctrl+F)",
        "pl": "Szukaj (Ctrl+F)", "cs": "Hledat (Ctrl+F)",
        "hu": "Keresés (Ctrl+F)", "ro": "Caută (Ctrl+F)",
        "nl": "Zoeken (Ctrl+F)", "sv": "Sök (Ctrl+F)",
        "nb": "Søk (Ctrl+F)", "da": "Søg (Ctrl+F)",
        "fi": "Haku (Ctrl+F)", "el": "Αναζήτηση (Ctrl+F)",
        "ar": "بحث (Ctrl+F)", "he": "חיפוש (Ctrl+F)",
        "tr": "Ara (Ctrl+F)", "hi": "खोजें (Ctrl+F)",
        "id": "Cari (Ctrl+F)", "ms": "Cari (Ctrl+F)",
        "fil": "Hanapin (Ctrl+F)", "vi": "Tìm (Ctrl+F)", "th": "ค้นหา (Ctrl+F)",
    },
    "export.open": {
        "zh-TW": "匯出", "ja": "エクスポート", "ko": "내보내기",
        "fr": "Exporter", "de": "Exportieren", "es": "Exportar", "it": "Esporta",
        "pt-BR": "Exportar", "pt": "Exportar", "ru": "Экспорт", "uk": "Експорт",
        "pl": "Eksport", "cs": "Export", "hu": "Exportálás", "ro": "Exportă",
        "nl": "Exporteren", "sv": "Exportera", "nb": "Eksporter", "da": "Eksporter",
        "fi": "Vie", "el": "Εξαγωγή", "ar": "تصدير", "he": "ייצוא", "tr": "Dışa aktar",
        "hi": "निर्यात", "id": "Ekspor", "ms": "Eksport", "fil": "I-export",
        "vi": "Xuất", "th": "ส่งออก",
    },
    "status.duration": {
        "zh-TW": "時長", "ja": "再生時間", "ko": "재생 시간",
        "fr": "Durée", "de": "Dauer", "es": "Duración", "it": "Durata",
        "pt-BR": "Duração", "pt": "Duração", "ru": "Длительность", "uk": "Тривалість",
        "pl": "Czas trwania", "cs": "Trvání", "hu": "Időtartam", "ro": "Durată",
        "nl": "Duur", "sv": "Varaktighet", "nb": "Varighet", "da": "Varighed",
        "fi": "Kesto", "el": "Διάρκεια", "ar": "المدة", "he": "משך", "tr": "Süre",
        "hi": "अवधि", "id": "Durasi", "ms": "Tempoh", "fil": "Tagal",
        "vi": "Thời lượng", "th": "ระยะเวลา",
    },
    "status.events": {
        "zh-TW": "事件", "ja": "イベント", "ko": "이벤트",
        "fr": "Événements", "de": "Ereignisse", "es": "Eventos", "it": "Eventi",
        "pt-BR": "Eventos", "pt": "Eventos", "ru": "События", "uk": "Події",
        "pl": "Zdarzenia", "cs": "Události", "hu": "Események", "ro": "Evenimente",
        "nl": "Gebeurt.", "sv": "Händelser", "nb": "Hendelser", "da": "Hændelser",
        "fi": "Tapahtumat", "el": "Συμβάντα", "ar": "الأحداث", "he": "אירועים",
        "tr": "Olaylar", "hi": "घटनाएं", "id": "Peristiwa", "ms": "Acara",
        "fil": "Mga Event", "vi": "Sự kiện", "th": "เหตุการณ์",
    },
    "status.state": {
        "zh-TW": "狀態", "ja": "状態", "ko": "상태",
        "fr": "État", "de": "Zustand", "es": "Estado", "it": "Stato",
        "pt-BR": "Estado", "pt": "Estado", "ru": "Состояние", "uk": "Стан",
        "pl": "Stan", "cs": "Stav", "hu": "Állapot", "ro": "Stare",
        "nl": "Status", "sv": "Status", "nb": "Status", "da": "Status",
        "fi": "Tila", "el": "Κατάσταση", "ar": "الحالة", "he": "מצב",
        "tr": "Durum", "hi": "स्थिति", "id": "Status", "ms": "Status",
        "fil": "Status", "vi": "Trạng thái", "th": "สถานะ",
    },
}

LOCALES = ["zh-TW", "ja", "ko", "fr", "de", "es", "it", "pt-BR", "pt",
           "ru", "uk", "pl", "cs", "hu", "ro", "nl", "sv", "nb", "da",
           "fi", "el", "ar", "he", "tr", "hi", "id", "ms", "fil", "vi", "th"]

def get_nested(d, path):
    parts = path.split(".")
    cur = d
    for p in parts:
        if not isinstance(cur, dict) or p not in cur:
            return None
        cur = cur[p]
    return cur

def set_nested(d, path, val):
    parts = path.split(".")
    cur = d
    for p in parts[:-1]:
        if p not in cur or not isinstance(cur[p], dict):
            cur[p] = {}
        cur = cur[p]
    cur[parts[-1]] = val

for lc in LOCALES:
    fn = L + lc + ".json"
    with open(fn, "r", encoding="utf-8") as f:
        d = json.load(f)

    added = []
    for key, translations in NEW.items():
        if get_nested(d, key) is None:
            translation = translations.get(lc, "")
            if translation:
                set_nested(d, key, translation)
                added.append(key)

    if added:
        with open(fn, "w", encoding="utf-8") as f:
            json.dump(d, f, ensure_ascii=False, indent=2)
            f.write("\n")
        print(f"[{lc}] +{len(added)} keys")
    else:
        print(f"[{lc}] no change")
