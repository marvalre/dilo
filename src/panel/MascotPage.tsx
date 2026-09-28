import { useEffect, useRef, useState } from "react";
import type { Settings } from "./api";
import { usePageVisible, useReducedMotion, useSettings } from "./hooks";
import { errMsg } from "./format";
import { PageHeader, Segmented, Switch } from "./ui";
import { MascotFace, SKINS, mascotBox, type MascotSize, type Skin } from "../shared/MascotFace";
import { useMascotAnim } from "../shared/useMascotAnim";

type Frame = { level: number; t: number; blink: boolean };

/** Fake voice: bursts of random levels with short pauses, like someone talking. */
function useFakeVoice(active: boolean) {
  const target = useRef(0);
  useEffect(() => {
    if (!active) {
      target.current = 0.35;
      return;
    }
    let pauseUntil = 0;
    const id = window.setInterval(() => {
      const now = performance.now();
      if (now < pauseUntil) {
        target.current = 0;
        return;
      }
      if (Math.random() < 0.04) pauseUntil = now + 350 + Math.random() * 600;
      target.current = 0.25 + Math.random() * 0.75;
    }, 110);
    return () => window.clearInterval(id);
  }, [active]);
  return target;
}

function Face({ skin, size, f, uid }: { skin: Skin; size: MascotSize; f: Frame; uid: string }) {
  return <MascotFace skin={skin} size={size} mode="listening" level={f.level} t={f.t} blink={f.blink} uid={uid} />;
}

/** Renders the face scaled up with a CSS transform, reserving the scaled layout box. */
function BigFace({ skin, f, uid, scale = 3 }: { skin: Skin; f: Frame; uid: string; scale?: number }) {
  const box = mascotBox(skin, "m");
  return (
    <div style={{ width: box.width * scale, height: box.height * scale }}>
      <div style={{ transform: `scale(${scale})`, transformOrigin: "0 0", width: box.width, height: box.height }}>
        <Face skin={skin} size="m" f={f} uid={uid} />
      </div>
    </div>
  );
}

export default function MascotPage() {
  const { settings, save } = useSettings();
  const reduced = useReducedMotion();
  const visible = usePageVisible();
  const target = useFakeVoice(!reduced);
  const live = useMascotAnim("listening", target, visible && !reduced);
  const f: Frame = reduced ? { level: 0.35, t: 420, blink: false } : live;
  const [error, setError] = useState<string | null>(null);

  if (!settings) return <PageHeader title="Monito" />;
  const skin = settings.mascot_skin;
  const update = (patch: Partial<Settings>) => {
    setError(null);
    save({ ...settings, ...patch }).catch((e) => setError(errMsg(e)));
  };

  return (
    <>
      <PageHeader title="Monito" subtitle="Te acompaña junto al cursor mientras dictas y reacciona a tu voz." />

      <section className={settings.mascot_enabled ? "stage" : "stage off"}>
        <div className="stage-half light">
          <BigFace skin={skin} f={f} uid="big-l" />
          <span className="stage-caption">Fondo claro</span>
        </div>
        <div className="stage-half dark">
          <BigFace skin={skin} f={f} uid="big-d" />
          <span className="stage-caption">Fondo oscuro</span>
        </div>
        <div className="stage-real">
          <span className="stage-real-label">Tamaño real</span>
          <div className="stage-real-demo">
            <span className="fake-text">
              Hola equipo, os escribo para<span className="fake-caret" />
            </span>
            <svg className="fake-cursor" width="14" height="20" viewBox="0 0 14 20" aria-hidden>
              <path d="M1 1v15.5l3.9-3.7 2.6 6 2.6-1.1-2.6-5.9H13Z" fill="#000" stroke="#fff" strokeWidth="1.2" strokeLinejoin="round" />
            </svg>
            <span className="fake-mascot">
              <Face skin={skin} size={settings.mascot_size} f={f} uid="real" />
            </span>
          </div>
        </div>
      </section>

      <div className="group">
        <div className="grow">
          <div className="grow-label">
            <span>Mostrar el monito al dictar</span>
            <span className="grow-sub">Si lo desactivas, Dicta sigue funcionando igual, sin la carita.</span>
          </div>
          <Switch label="Mostrar el monito al dictar" checked={settings.mascot_enabled} onChange={(v) => update({ mascot_enabled: v })} />
        </div>
        <div className="grow">
          <div className="grow-label">
            <span>Tamaño</span>
          </div>
          <Segmented
            label="Tamaño"
            value={settings.mascot_size}
            onChange={(v) => update({ mascot_size: v })}
            options={[
              { value: "s", label: "Pequeño" },
              { value: "m", label: "Mediano" },
              { value: "l", label: "Grande" },
            ]}
          />
        </div>
      </div>

      <h2 className="section-label">Estilo</h2>
      <div className="skins" role="radiogroup" aria-label="Estilo del monito">
        {SKINS.map((s) => (
          <button
            key={s.id}
            role="radio"
            aria-checked={skin === s.id}
            className={skin === s.id ? "skin active" : "skin"}
            onClick={() => update({ mascot_skin: s.id })}
          >
            <span className="skin-preview">
              <BigFace skin={s.id} f={skin === s.id ? f : { level: 0.3, t: 420, blink: false }} uid={`skin-${s.id}`} scale={1.6} />
            </span>
            <span className="skin-name">{s.name}</span>
            <span className="skin-desc">{s.description}</span>
          </button>
        ))}
      </div>

      {error && <p className="error-text">{error}</p>}
    </>
  );
}
