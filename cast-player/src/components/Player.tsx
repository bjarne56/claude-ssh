import { useEffect, useRef } from "react";
import type { usePlayer } from "../usePlayer";

type PlayerHook = ReturnType<typeof usePlayer>;

interface PlayerProps {
  player: PlayerHook;
}

export function Player({ player }: PlayerProps) {
  const divRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (divRef.current) {
      return player.initTerminal(divRef.current);
    }
  }, [player.initTerminal]);

  return (
    <div className="player-container">
      <div
        ref={divRef}
        className="terminal-wrapper"
        style={{ flex: 1, overflow: "hidden" }}
      />
    </div>
  );
}