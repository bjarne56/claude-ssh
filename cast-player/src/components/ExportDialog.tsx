import { useState } from "react";
import { save } from "@tauri-apps/plugin-dialog";
import { exportCommandsCsv, exportCommandsJson, copyCastFile } from "../tauri-api";
import { useTranslation } from "../i18n";

interface ExportDialogProps {
  sessionDir: string;
  sessionId: string;
  onClose: () => void;
}

export function ExportDialog({ sessionDir, sessionId, onClose }: ExportDialogProps) {
  const { t } = useTranslation();
  const [status, setStatus] = useState("");
  const [exporting, setExporting] = useState(false);

  const handleExport = async (type: "csv" | "json" | "cast") => {
    const ext = type === "cast" ? "cast" : type;
    // 弹保存对话框, 用户选导出位置
    const filterName = type === "cast" ? "Asciinema cast" : type.toUpperCase();
    let outputPath: string | null;
    try {
      outputPath = await save({
        title: "选择导出位置",
        defaultPath: `${sessionId}.${ext}`,
        filters: [{ name: filterName, extensions: [ext] }],
      });
    } catch (err) {
      setStatus(`${t("export.failed")} ${err}`);
      return;
    }
    if (!outputPath) return; // 用户取消

    setExporting(true);
    setStatus(t("export.exporting"));
    try {
      let msg = "";
      switch (type) {
        case "csv":
          msg = await exportCommandsCsv(sessionDir, outputPath);
          break;
        case "json":
          msg = await exportCommandsJson(sessionDir, outputPath);
          break;
        case "cast":
          msg = await copyCastFile(sessionDir, outputPath);
          break;
      }
      setStatus(msg);
    } catch (err) {
      setStatus(`${t("export.failed")} ${err}`);
    } finally {
      setExporting(false);
    }
  };

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="modal-box" onClick={(e) => e.stopPropagation()}>
        <h3>{t("export.title")}: {sessionId}</h3>
        <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
          <button
            onClick={() => handleExport("csv")}
            disabled={exporting}
            className="primary"
          >
            {t("export.csv")}
          </button>
          <button
            onClick={() => handleExport("json")}
            disabled={exporting}
            className="primary"
          >
            {t("export.json")}
          </button>
          <button
            onClick={() => handleExport("cast")}
            disabled={exporting}
            className="primary"
          >
            {t("export.cast")}
          </button>
        </div>
        {status && (
          <div style={{ fontSize: 12, color: "var(--text-muted)", wordBreak: "break-all" }}>
            {status}
          </div>
        )}
        <div className="actions">
          <button onClick={onClose}>{t("export.close")}</button>
        </div>
      </div>
    </div>
  );
}