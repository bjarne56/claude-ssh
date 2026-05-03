// 31 国系统菜单本地化
//
// macOS 左上角的应用菜单 / 文件菜单 / 编辑菜单 / 视图菜单 / 窗口菜单 / 帮助菜单
// 每个 menu key 用一个 HashMap<locale, label> 索引

use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct MenuLabels {
    pub app: AppMenu,
    pub file: FileMenu,
    pub edit: EditMenu,
    pub view: ViewMenu,
    pub window: WindowMenu,
    pub help: HelpMenu,
}

#[derive(Debug, Clone)]
pub struct AppMenu {
    pub about: String,
    pub services: String,
    pub hide: String,
    pub hide_others: String,
    pub show_all: String,
    pub quit: String,
}

#[derive(Debug, Clone)]
pub struct FileMenu {
    pub title: String,
    pub close_window: String,
}

#[derive(Debug, Clone)]
pub struct EditMenu {
    pub title: String,
    pub undo: String,
    pub redo: String,
    pub cut: String,
    pub copy: String,
    pub paste: String,
    pub select_all: String,
}

#[derive(Debug, Clone)]
pub struct ViewMenu {
    pub title: String,
    pub fullscreen: String,
}

#[derive(Debug, Clone)]
pub struct WindowMenu {
    pub title: String,
    pub minimize: String,
    pub zoom: String,
    pub bring_to_front: String,
}

#[derive(Debug, Clone)]
pub struct HelpMenu {
    pub title: String,
}

/// 语言菜单的 title (出现在菜单栏, 帮助菜单前)
pub fn language_menu_title(locale: &str) -> &'static str {
    match locale {
        "zh-CN" => "语言", "zh-TW" => "語言", "ja" => "言語", "ko" => "언어",
        "fr" => "Langue", "de" => "Sprache", "es" => "Idioma", "it" => "Lingua",
        "pt-BR" | "pt" => "Idioma", "ru" => "Язык", "uk" => "Мова",
        "pl" => "Język", "cs" => "Jazyk", "hu" => "Nyelv", "ro" => "Limbă",
        "nl" => "Taal", "sv" => "Språk", "nb" => "Språk", "da" => "Sprog",
        "fi" => "Kieli", "el" => "Γλώσσα", "ar" => "اللغة", "he" => "שפה",
        "tr" => "Dil", "hi" => "भाषा", "id" => "Bahasa", "ms" => "Bahasa",
        "fil" => "Wika", "vi" => "Ngôn ngữ", "th" => "ภาษา",
        _ => "Language",
    }
}

/// 31 国 locale 列表 (code, 显示名)
pub fn supported_locales() -> Vec<(&'static str, &'static str)> {
    vec![
        ("en", "English"),
        ("zh-CN", "简体中文"),
        ("zh-TW", "繁體中文"),
        ("ja", "日本語"),
        ("ko", "한국어"),
        ("fr", "Français"),
        ("de", "Deutsch"),
        ("es", "Español"),
        ("it", "Italiano"),
        ("pt-BR", "Português (BR)"),
        ("pt", "Português"),
        ("ru", "Русский"),
        ("uk", "Українська"),
        ("pl", "Polski"),
        ("cs", "Čeština"),
        ("hu", "Magyar"),
        ("ro", "Română"),
        ("nl", "Nederlands"),
        ("sv", "Svenska"),
        ("nb", "Norsk Bokmål"),
        ("da", "Dansk"),
        ("fi", "Suomi"),
        ("el", "Ελληνικά"),
        ("ar", "العربية"),
        ("he", "עברית"),
        ("tr", "Türkçe"),
        ("hi", "हिन्दी"),
        ("id", "Indonesia"),
        ("ms", "Melayu"),
        ("fil", "Filipino"),
        ("vi", "Tiếng Việt"),
        ("th", "ไทย"),
    ]
}

/// 拿到指定 locale 的菜单 labels (locale 不存在则 fallback en)
pub fn labels_for(locale: &str) -> MenuLabels {
    LABELS
        .get(locale)
        .or_else(|| LABELS.get("en"))
        .cloned()
        .unwrap()
}

lazy_static::lazy_static! {
    static ref LABELS: HashMap<&'static str, MenuLabels> = build_labels();
}

fn build_labels() -> HashMap<&'static str, MenuLabels> {
    let mut m = HashMap::new();

    macro_rules! L {
        ($locale:expr,
         $about:expr, $services:expr, $hide:expr, $hide_others:expr, $show_all:expr, $quit:expr,
         $file:expr, $close:expr,
         $edit:expr, $undo:expr, $redo:expr, $cut:expr, $copy:expr, $paste:expr, $select_all:expr,
         $view:expr, $fullscreen:expr,
         $window:expr, $minimize:expr, $zoom:expr, $bring_front:expr,
         $help:expr
        ) => {
            m.insert($locale, MenuLabels {
                app: AppMenu { about: $about.into(), services: $services.into(), hide: $hide.into(),
                               hide_others: $hide_others.into(), show_all: $show_all.into(), quit: $quit.into() },
                file: FileMenu { title: $file.into(), close_window: $close.into() },
                edit: EditMenu { title: $edit.into(), undo: $undo.into(), redo: $redo.into(),
                                 cut: $cut.into(), copy: $copy.into(), paste: $paste.into(), select_all: $select_all.into() },
                view: ViewMenu { title: $view.into(), fullscreen: $fullscreen.into() },
                window: WindowMenu { title: $window.into(), minimize: $minimize.into(),
                                     zoom: $zoom.into(), bring_to_front: $bring_front.into() },
                help: HelpMenu { title: $help.into() },
            });
        };
    }

    L!("en", "About Cast Player", "Services", "Hide Cast Player", "Hide Others", "Show All", "Quit Cast Player",
       "File", "Close Window",
       "Edit", "Undo", "Redo", "Cut", "Copy", "Paste", "Select All",
       "View", "Toggle Fullscreen",
       "Window", "Minimize", "Zoom", "Bring All to Front",
       "Help");

    L!("zh-CN", "关于 Cast Player", "服务", "隐藏 Cast Player", "隐藏其他", "全部显示", "退出 Cast Player",
       "文件", "关闭窗口",
       "编辑", "撤销", "重做", "剪切", "拷贝", "粘贴", "全选",
       "显示", "切换全屏",
       "窗口", "最小化", "缩放", "前置全部窗口",
       "帮助");

    L!("zh-TW", "關於 Cast Player", "服務", "隱藏 Cast Player", "隱藏其他", "全部顯示", "結束 Cast Player",
       "檔案", "關閉視窗",
       "編輯", "復原", "重做", "剪下", "拷貝", "貼上", "全選",
       "顯示方式", "切換全螢幕",
       "視窗", "最小化", "縮放", "全部前置",
       "輔助說明");

    L!("ja", "Cast Player について", "サービス", "Cast Player を非表示", "ほかを非表示", "すべて表示", "Cast Player を終了",
       "ファイル", "ウインドウを閉じる",
       "編集", "取り消す", "やり直す", "カット", "コピー", "ペースト", "すべてを選択",
       "表示", "フルスクリーン切替",
       "ウインドウ", "しまう", "拡大/縮小", "すべてを手前に移動",
       "ヘルプ");

    L!("ko", "Cast Player 정보", "서비스", "Cast Player 숨기기", "기타 가리기", "모두 보기", "Cast Player 종료",
       "파일", "윈도우 닫기",
       "편집", "실행 취소", "다시 실행", "잘라내기", "복사하기", "붙여넣기", "전체 선택",
       "보기", "전체 화면 전환",
       "윈도우", "최소화", "확대/축소", "모두 앞으로 가져오기",
       "도움말");

    L!("fr", "À propos de Cast Player", "Services", "Masquer Cast Player", "Masquer les autres", "Tout afficher", "Quitter Cast Player",
       "Fichier", "Fermer la fenêtre",
       "Édition", "Annuler", "Rétablir", "Couper", "Copier", "Coller", "Tout sélectionner",
       "Affichage", "Basculer en plein écran",
       "Fenêtre", "Réduire", "Zoom", "Tout ramener au premier plan",
       "Aide");

    L!("de", "Über Cast Player", "Dienste", "Cast Player ausblenden", "Andere ausblenden", "Alle einblenden", "Cast Player beenden",
       "Datei", "Fenster schließen",
       "Bearbeiten", "Rückgängig", "Wiederherstellen", "Ausschneiden", "Kopieren", "Einsetzen", "Alles auswählen",
       "Darstellung", "Vollbildmodus",
       "Fenster", "Im Dock ablegen", "Zoomen", "Alle nach vorne bringen",
       "Hilfe");

    L!("es", "Acerca de Cast Player", "Servicios", "Ocultar Cast Player", "Ocultar otros", "Mostrar todo", "Salir de Cast Player",
       "Archivo", "Cerrar ventana",
       "Edición", "Deshacer", "Rehacer", "Cortar", "Copiar", "Pegar", "Seleccionar todo",
       "Vista", "Pantalla completa",
       "Ventana", "Minimizar", "Zoom", "Traer todo al frente",
       "Ayuda");

    L!("it", "Informazioni su Cast Player", "Servizi", "Nascondi Cast Player", "Nascondi altri", "Mostra tutto", "Esci da Cast Player",
       "File", "Chiudi finestra",
       "Modifica", "Annulla", "Ripristina", "Taglia", "Copia", "Incolla", "Seleziona tutto",
       "Vista", "Schermo intero",
       "Finestra", "Riduci a icona", "Zoom", "Porta tutto in primo piano",
       "Aiuto");

    L!("pt-BR", "Sobre o Cast Player", "Serviços", "Ocultar Cast Player", "Ocultar Outros", "Mostrar Todos", "Encerrar Cast Player",
       "Arquivo", "Fechar Janela",
       "Editar", "Desfazer", "Refazer", "Recortar", "Copiar", "Colar", "Selecionar Tudo",
       "Visualização", "Tela Cheia",
       "Janela", "Minimizar", "Zoom", "Trazer Tudo para Frente",
       "Ajuda");

    L!("pt", "Acerca do Cast Player", "Serviços", "Ocultar Cast Player", "Ocultar Outros", "Mostrar Todos", "Sair do Cast Player",
       "Ficheiro", "Fechar Janela",
       "Editar", "Anular", "Refazer", "Cortar", "Copiar", "Colar", "Seleccionar Tudo",
       "Visualização", "Ecrã Inteiro",
       "Janela", "Minimizar", "Zoom", "Trazer Tudo para Primeiro Plano",
       "Ajuda");

    L!("ru", "О Cast Player", "Службы", "Скрыть Cast Player", "Скрыть остальные", "Показать все", "Завершить Cast Player",
       "Файл", "Закрыть окно",
       "Правка", "Отменить", "Повторить", "Вырезать", "Скопировать", "Вставить", "Выбрать все",
       "Вид", "Полный экран",
       "Окно", "Свернуть", "Увеличить", "Все окна на передний план",
       "Справка");

    L!("uk", "Про Cast Player", "Служби", "Сховати Cast Player", "Сховати інші", "Показати все", "Вийти з Cast Player",
       "Файл", "Закрити вікно",
       "Редагування", "Скасувати", "Повернути", "Вирізати", "Копіювати", "Вставити", "Вибрати все",
       "Вигляд", "Повний екран",
       "Вікно", "Згорнути", "Збільшити", "Всі вікна на передній план",
       "Довідка");

    L!("pl", "Cast Player – informacje", "Usługi", "Ukryj Cast Player", "Ukryj inne", "Pokaż wszystko", "Zakończ Cast Player",
       "Plik", "Zamknij okno",
       "Edycja", "Cofnij", "Ponów", "Wytnij", "Kopiuj", "Wklej", "Zaznacz wszystko",
       "Widok", "Pełny ekran",
       "Okno", "Schowaj", "Powiększ", "Wszystkie na wierzch",
       "Pomoc");

    L!("cs", "O Cast Player", "Služby", "Skrýt Cast Player", "Skrýt ostatní", "Zobrazit vše", "Ukončit Cast Player",
       "Soubor", "Zavřít okno",
       "Úpravy", "Zpět", "Znovu", "Vyjmout", "Kopírovat", "Vložit", "Vybrat vše",
       "Zobrazení", "Celá obrazovka",
       "Okno", "Skrýt", "Přiblížit", "Přenést vše do popředí",
       "Nápověda");

    L!("hu", "A Cast Player névjegye", "Szolgáltatások", "Cast Player elrejtése", "Többi elrejtése", "Mind megjelenítése", "Kilépés a Cast Player-ből",
       "Fájl", "Ablak bezárása",
       "Szerkesztés", "Visszavonás", "Újra", "Kivágás", "Másolás", "Beillesztés", "Mind kijelölése",
       "Nézet", "Teljes képernyő",
       "Ablak", "Kis méret", "Nagyítás", "Mind előrehozása",
       "Súgó");

    L!("ro", "Despre Cast Player", "Servicii", "Ascunde Cast Player", "Ascunde celelalte", "Arată toate", "Părăsește Cast Player",
       "Fișier", "Închide fereastra",
       "Editare", "Anulează", "Refă", "Decupează", "Copiază", "Lipește", "Selectează tot",
       "Vizualizare", "Ecran complet",
       "Fereastră", "Minimizează", "Zoom", "Adu tot în prim-plan",
       "Ajutor");

    L!("nl", "Over Cast Player", "Diensten", "Verberg Cast Player", "Verberg andere", "Toon alles", "Stop Cast Player",
       "Bestand", "Sluit venster",
       "Wijzig", "Herstel", "Opnieuw", "Knip", "Kopieer", "Plak", "Selecteer alles",
       "Weergave", "Volledig scherm",
       "Venster", "Minimaliseer", "Zoom", "Alles naar voorgrond",
       "Help");

    L!("sv", "Om Cast Player", "Tjänster", "Göm Cast Player", "Göm övriga", "Visa alla", "Avsluta Cast Player",
       "Arkiv", "Stäng fönster",
       "Redigera", "Ångra", "Gör om", "Klipp ut", "Kopiera", "Klistra in", "Markera allt",
       "Visa", "Helskärm",
       "Fönster", "Minimera", "Zooma", "Lägg alla överst",
       "Hjälp");

    L!("nb", "Om Cast Player", "Tjenester", "Skjul Cast Player", "Skjul andre", "Vis alle", "Avslutt Cast Player",
       "Arkiv", "Lukk vindu",
       "Rediger", "Angre", "Gjør om", "Klipp ut", "Kopier", "Lim inn", "Velg alt",
       "Vis", "Fullskjerm",
       "Vindu", "Minimer", "Zoom", "Plasser alle fremst",
       "Hjelp");

    L!("da", "Om Cast Player", "Tjenester", "Skjul Cast Player", "Skjul andre", "Vis alle", "Slut Cast Player",
       "Arkiv", "Luk vindue",
       "Rediger", "Fortryd", "Gentag", "Klip", "Kopiér", "Sæt ind", "Vælg alle",
       "Vis", "Fuld skærm",
       "Vindue", "Minimer", "Zoom", "Bring alle øverst",
       "Hjælp");

    L!("fi", "Tietoja Cast Playerista", "Palvelut", "Piilota Cast Player", "Piilota muut", "Näytä kaikki", "Lopeta Cast Player",
       "Arkisto", "Sulje ikkuna",
       "Muokkaus", "Kumoa", "Tee uudelleen", "Leikkaa", "Kopioi", "Sijoita", "Valitse kaikki",
       "Näytä", "Kokoruutu",
       "Ikkuna", "Pienennä", "Zoomaa", "Tuo kaikki eteen",
       "Ohjeet");

    L!("el", "Σχετικά με το Cast Player", "Υπηρεσίες", "Απόκρυψη Cast Player", "Απόκρυψη άλλων", "Εμφάνιση όλων", "Τερματισμός Cast Player",
       "Αρχείο", "Κλείσιμο παραθύρου",
       "Επεξεργασία", "Αναίρεση", "Επανάληψη", "Αποκοπή", "Αντιγραφή", "Επικόλληση", "Επιλογή όλων",
       "Προβολή", "Πλήρης οθόνη",
       "Παράθυρο", "Ελαχιστοποίηση", "Ζουμ", "Όλα μπροστά",
       "Βοήθεια");

    L!("ar", "حول Cast Player", "الخدمات", "إخفاء Cast Player", "إخفاء أخرى", "إظهار الكل", "الخروج من Cast Player",
       "ملف", "إغلاق النافذة",
       "تحرير", "تراجع", "إعادة", "قص", "نسخ", "لصق", "تحديد الكل",
       "عرض", "ملء الشاشة",
       "نافذة", "تصغير", "تكبير/تصغير", "إحضار الكل للأمام",
       "مساعدة");

    L!("he", "אודות Cast Player", "שירותים", "הסתר את Cast Player", "הסתר אחרים", "הצג הכול", "צא מ-Cast Player",
       "קובץ", "סגור חלון",
       "עריכה", "בטל", "בצע שוב", "גזור", "העתק", "הדבק", "בחר הכל",
       "תצוגה", "מסך מלא",
       "חלון", "מזער", "מרבי", "הבא הכל לחזית",
       "עזרה");

    L!("tr", "Cast Player Hakkında", "Hizmetler", "Cast Player'ı Gizle", "Diğerlerini Gizle", "Tümünü Göster", "Cast Player'dan Çık",
       "Dosya", "Pencereyi Kapat",
       "Düzenle", "Geri Al", "Yinele", "Kes", "Kopyala", "Yapıştır", "Tümünü Seç",
       "Görünüm", "Tam Ekran",
       "Pencere", "Simge Durumuna Küçült", "Yakınlaştır", "Tümünü Öne Getir",
       "Yardım");

    L!("hi", "Cast Player के बारे में", "सेवाएं", "Cast Player छिपाएं", "अन्य छिपाएं", "सब दिखाएं", "Cast Player से बाहर निकलें",
       "फ़ाइल", "विंडो बंद करें",
       "संपादित", "वापस", "पुनः", "काटें", "कॉपी", "पेस्ट", "सभी चुनें",
       "देखें", "पूर्ण स्क्रीन",
       "विंडो", "छोटा करें", "ज़ूम", "सभी सामने लाएं",
       "मदद");

    L!("id", "Tentang Cast Player", "Layanan", "Sembunyikan Cast Player", "Sembunyikan Lainnya", "Tampilkan Semua", "Keluar dari Cast Player",
       "Berkas", "Tutup Jendela",
       "Edit", "Urungkan", "Ulangi", "Potong", "Salin", "Tempel", "Pilih Semua",
       "Tampilan", "Layar Penuh",
       "Jendela", "Minimalkan", "Zoom", "Bawa Semua ke Depan",
       "Bantuan");

    L!("ms", "Tentang Cast Player", "Servis", "Sembunyikan Cast Player", "Sembunyikan Lain", "Tunjukkan Semua", "Keluar Cast Player",
       "Fail", "Tutup Tetingkap",
       "Sunting", "Buat Asal", "Buat Semula", "Potong", "Salin", "Tampal", "Pilih Semua",
       "Lihat", "Skrin Penuh",
       "Tetingkap", "Minimumkan", "Zum", "Bawa Semua ke Depan",
       "Bantuan");

    L!("fil", "Tungkol sa Cast Player", "Mga Serbisyo", "Itago ang Cast Player", "Itago ang Iba", "Ipakita Lahat", "Lumabas sa Cast Player",
       "File", "Isara ang Window",
       "I-edit", "I-undo", "I-redo", "Gupitin", "Kopyahin", "I-paste", "Piliin Lahat",
       "Tingnan", "Buong Screen",
       "Window", "I-minimize", "Mag-zoom", "Dalhin Lahat sa Harap",
       "Tulong");

    L!("vi", "Giới thiệu về Cast Player", "Dịch vụ", "Ẩn Cast Player", "Ẩn các phần khác", "Hiện tất cả", "Thoát Cast Player",
       "Tệp", "Đóng cửa sổ",
       "Sửa", "Hoàn tác", "Làm lại", "Cắt", "Sao chép", "Dán", "Chọn tất cả",
       "Xem", "Toàn màn hình",
       "Cửa sổ", "Thu nhỏ", "Phóng to", "Đưa tất cả ra trước",
       "Trợ giúp");

    L!("th", "เกี่ยวกับ Cast Player", "บริการ", "ซ่อน Cast Player", "ซ่อนอื่นๆ", "แสดงทั้งหมด", "ออกจาก Cast Player",
       "ไฟล์", "ปิดหน้าต่าง",
       "แก้ไข", "เลิกทำ", "ทำซ้ำ", "ตัด", "คัดลอก", "วาง", "เลือกทั้งหมด",
       "มุมมอง", "เต็มหน้าจอ",
       "หน้าต่าง", "ย่อ", "ซูม", "นำทั้งหมดไปข้างหน้า",
       "วิธีใช้");

    m
}

/// 从环境变量 LANG 推断 locale (e.g. "zh_CN.UTF-8" → "zh-CN")
pub fn detect_system_locale() -> String {
    if let Ok(env_locale) = std::env::var("LANG") {
        let core = env_locale.split('.').next().unwrap_or("");
        let normalized = core.replace('_', "-");
        if LABELS.contains_key(normalized.as_str()) {
            return normalized;
        }
        // 前缀 fallback (如 "zh-HK" → "zh-CN")
        if let Some(prefix) = normalized.split('-').next() {
            for key in LABELS.keys() {
                if key.starts_with(prefix) {
                    return key.to_string();
                }
            }
        }
    }
    "en".to_string()
}
