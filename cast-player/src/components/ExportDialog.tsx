import { useState } from "react";
import { exportCommandsCsv, exportCommandsJson, copyCastFile } from "../tauri-api";

interface ExportDialogProps {
  sessionDir: string;
  sessionId: string;
  onClose: () => void;
}

export function ExportDialog({ sessionDir, sessionId, onClose }: ExportDialogProps) {
  const [status, setStatus] = useState("");
  const [exporting, setExporting] = useState(false);

  const handleExport = async (type: "csv" | "json" | "cast") => {
    setExporting(true);
    setStatus("导出中...");
    try {
      const ext = type === "cast" ? "cast" : type;
      const outputPath = `${sessionDir}/${sessionId}.${ext}`;
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
      setStatus(`导出失败: ${err}`);
    } finally {
      setExporting(false);
    }
  };

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="modal-box" onClick={(e) => e.stopPropagation()}>
        <h3>导出会话: {sessionId}</h3>
        <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
          <button
            onClick={() => handleExport("csv")}
            disabled={exporting}
            className="primary"
          >
            导出命令清单 (CSV)
          </button>
          <button
            onClick={() => handleExport("json")}
            disabled={exporting}
            className="primary"
          >
            导出命令清单 (JSON)
          </button>
          <button
            onClick={() => handleExport("cast")}
            disabled={exporting}
            className="primary"
          >
            复制原始录像文件 (.cast)
          </button>
        </div>
        {status && (
          <div style={{ fontSize: 12, color: "var(--text-muted)", wordBreak: "break-all" }}>
            {status}
          </div>
        )}
        <div className="actions">
          <button onClick={onClose}>关闭</button>
        </div>
      </div>
    </div>
  );
}