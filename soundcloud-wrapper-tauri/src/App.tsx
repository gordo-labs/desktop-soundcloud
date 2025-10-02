import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

const DISCOGS_AMBIGUITY_EVENT = "app://discogs/lookup-ambiguous";
const JOB_PROGRESS_EVENT = "app://jobs/progress";
const DEFAULT_PAGE_SIZE = 50;

type Nullable<T> = T | null;

type LibraryStatusRow = {
  trackId: string;
  title?: string | null;
  artist?: string | null;
  album?: string | null;
  liked: boolean;
  matched: boolean;
  hasLocalFile: boolean;
  localAvailable: boolean;
  inRekordbox: boolean;
  discogsStatus?: string | null;
  discogsReleaseId?: string | null;
  discogsConfidence?: number | null;
  discogsCheckedAt?: string | null;
  discogsMessage?: string | null;
  musicbrainzStatus?: string | null;
  musicbrainzReleaseId?: string | null;
  musicbrainzConfidence?: number | null;
  musicbrainzCheckedAt?: string | null;
  musicbrainzMessage?: string | null;
  soundcloudPermalinkUrl?: string | null;
  soundcloudLikedAt?: string | null;
  localLocation?: string | null;
};

type LibraryStatusPage = {
  rows: LibraryStatusRow[];
  total: number;
  limit: number;
  offset: number;
};

type FilterState = {
  missingAssetsOnly: boolean;
  unresolvedDiscogsOnly: boolean;
  likedOnly: boolean;
  rekordboxOnly: boolean;
};

type DiscogsCandidatePayload = {
  matchId: string;
  releaseId?: string | null;
  score?: number | null;
  rawPayload?: Record<string, unknown> | null;
};

type DiscogsAmbiguityEvent = {
  trackId: string;
  query?: string;
  candidates: Record<string, unknown>[];
};

type DiscogsCandidate = {
  matchId: string;
  releaseId?: string;
  score?: number;
  title?: string;
  year?: number;
  country?: string;
  thumb?: string;
  resourceUrl?: string;
  rawPayload: Record<string, unknown>;
};

type JobProgressPayload = {
  id?: string;
  label?: string;
  state?: string;
  completed?: number;
  total?: number;
  message?: string;
};

type JobRecord = {
  id: string;
  label: string;
  state: string;
  completed: number;
  total?: number;
  message?: string;
  updatedAt: number;
};

type AsyncState<T> =
  | { status: "idle" }
  | { status: "loading" }
  | { status: "error"; message: string }
  | { status: "ready"; data: T };

const normalizeCandidate = (candidate: DiscogsCandidatePayload): DiscogsCandidate => {
  const raw = candidate.rawPayload ?? {};
  const title = typeof raw.title === "string" ? raw.title : undefined;
  const year = typeof raw.year === "number" ? raw.year : undefined;
  const country = typeof raw.country === "string" ? raw.country : undefined;
  const thumb = typeof raw.thumb === "string" ? raw.thumb : undefined;
  const resourceUrl = typeof raw.resourceUrl === "string" ? raw.resourceUrl : undefined;
  const score = typeof candidate.score === "number" ? candidate.score : undefined;
  const releaseId = candidate.releaseId ?? (typeof raw.id === "string" ? raw.id : undefined);

  return {
    matchId: candidate.matchId,
    releaseId: releaseId ?? undefined,
    score,
    title,
    year,
    country,
    thumb,
    resourceUrl,
    rawPayload: raw,
  };
};

const formatDate = (value?: string | null) => {
  if (!value) {
    return "Sin datos";
  }
  const parsed = new Date(value);
  if (Number.isNaN(parsed.getTime())) {
    return value;
  }
  return parsed.toLocaleString();
};

const Badge = ({ label, variant }: { label: string; variant: "primary" | "success" | "warning" | "neutral" }) => (
  <span className={`badge badge--${variant}`}>{label}</span>
);

const Checkbox = ({
  id,
  label,
  checked,
  onChange,
}: {
  id: string;
  label: string;
  checked: boolean;
  onChange: (value: boolean) => void;
}) => (
  <label className="filter-toggle" htmlFor={id}>
    <input
      id={id}
      type="checkbox"
      checked={checked}
      onChange={(event) => onChange(event.target.checked)}
    />
    <span>{label}</span>
  </label>
);

const getErrorMessage = (error: unknown) => {
  if (typeof error === "string") {
    return error;
  }
  if (error instanceof Error) {
    return error.message;
  }
  try {
    return JSON.stringify(error);
  } catch (_jsonError) {
    return "Operación fallida";
  }
};

const App = () => {
  const [filters, setFilters] = useState<FilterState>({
    missingAssetsOnly: false,
    unresolvedDiscogsOnly: false,
    likedOnly: false,
    rekordboxOnly: false,
  });
  const [tracks, setTracks] = useState<LibraryStatusRow[]>([]);
  const [totalTracks, setTotalTracks] = useState(0);
  const [currentOffset, setCurrentOffset] = useState(0);
  const [loadingTracks, setLoadingTracks] = useState(false);
  const [trackError, setTrackError] = useState<string | null>(null);
  const [selectedTrackId, setSelectedTrackId] = useState<Nullable<string>>(null);
  const [candidateCache, setCandidateCache] = useState<Record<string, DiscogsCandidate[]>>({});
  const [candidateState, setCandidateState] = useState<AsyncState<DiscogsCandidate[]>>({ status: "idle" });
  const [jobs, setJobs] = useState<Record<string, JobRecord>>({});
  const [statusMessage, setStatusMessage] = useState<Nullable<{ type: "success" | "error" | "info"; text: string }>>(null);

  useEffect(() => {
    if (!statusMessage) {
      return;
    }
    const timeout = window.setTimeout(() => setStatusMessage(null), 5000);
    return () => window.clearTimeout(timeout);
  }, [statusMessage]);

  const filterPayload = useMemo(
    () => ({
      missingAssetsOnly: filters.missingAssetsOnly,
      unresolvedDiscogsOnly: filters.unresolvedDiscogsOnly,
      likedOnly: filters.likedOnly,
      rekordboxOnly: filters.rekordboxOnly,
      limit: DEFAULT_PAGE_SIZE,
    }),
    [filters]
  );

  const fetchTracks = useCallback(
    async (offset: number, replace: boolean) => {
      setLoadingTracks(true);
      setTrackError(null);
      try {
        const response = await invoke<LibraryStatusPage>("list_library_status", {
          filter: {
            ...filterPayload,
            offset,
          },
        });
        setTracks((previous) => (replace ? response.rows : [...previous, ...response.rows]));
        setTotalTracks(response.total ?? response.rows.length);
        const nextOffset = (response.offset ?? offset) + response.rows.length;
        setCurrentOffset(nextOffset);
        if (replace) {
          setCandidateState({ status: "idle" });
        }
      } catch (error) {
        setTrackError(getErrorMessage(error));
      } finally {
        setLoadingTracks(false);
      }
    },
    [filterPayload]
  );

  useEffect(() => {
    fetchTracks(0, true).catch((error) => {
      setTrackError(getErrorMessage(error));
    });
  }, [fetchTracks]);

  useEffect(() => {
    if (!selectedTrackId && tracks.length > 0) {
      setSelectedTrackId(tracks[0].trackId);
      return;
    }
    if (selectedTrackId && !tracks.some((row) => row.trackId === selectedTrackId)) {
      setSelectedTrackId(tracks.length > 0 ? tracks[0].trackId : null);
    }
  }, [selectedTrackId, tracks]);

  const attachDiscogsListener = useCallback(async () => {
    try {
      const unlistenPromise = listen<DiscogsAmbiguityEvent>(DISCOGS_AMBIGUITY_EVENT, (event) => {
        const payload = event.payload;
        if (!payload || !payload.trackId) {
          return;
        }
        setCandidateCache((previous) => ({
          ...previous,
          [payload.trackId]: (payload.candidates || []).map((candidate) => {
            const rawId =
              typeof candidate.releaseId === "string"
                ? candidate.releaseId
                : typeof candidate.releaseId === "number"
                ? candidate.releaseId.toString()
                : typeof candidate.id === "string"
                ? candidate.id
                : typeof candidate.id === "number"
                ? candidate.id.toString()
                : undefined;
            const rawScore =
              typeof candidate.score === "number"
                ? candidate.score
                : typeof candidate.score === "string"
                ? Number.parseFloat(candidate.score)
                : undefined;
            return normalizeCandidate({
              matchId: payload.trackId,
              rawPayload: candidate,
              releaseId: rawId,
              score: rawScore,
            });
          }),
        }));
      });
      return unlistenPromise;
    } catch (error) {
      console.warn("No se pudo suscribir a eventos de Discogs", error);
      return null;
    }
  }, []);

  useEffect(() => {
    let unlisten: UnlistenFn | null = null;
    attachDiscogsListener().then((dispose) => {
      unlisten = dispose ?? null;
    });
    return () => {
      if (unlisten) {
        unlisten();
      }
    };
  }, [attachDiscogsListener]);

  useEffect(() => {
    const attachJobListener = async () => {
      try {
        const dispose = await listen<JobProgressPayload>(JOB_PROGRESS_EVENT, (event) => {
          const payload = event.payload;
          if (!payload) {
            return;
          }
          const id = payload.id || payload.label || "background-job";
          setJobs((previous) => {
            const next: Record<string, JobRecord> = { ...previous };
            const existing = next[id];
            const record: JobRecord = {
              id,
              label: payload.label || existing?.label || "Tarea en segundo plano",
              state: payload.state || existing?.state || "running",
              completed: payload.completed ?? existing?.completed ?? 0,
              total: payload.total ?? existing?.total,
              message: payload.message ?? existing?.message,
              updatedAt: Date.now(),
            };
            next[id] = record;
            if (record.state === "completed" || record.state === "success") {
              // Remove completed jobs after a short delay
              setTimeout(() => {
                setJobs((current) => {
                  const clone = { ...current };
                  delete clone[id];
                  return clone;
                });
              }, 4000);
            }
            return next;
          });
        });
        return dispose;
      } catch (error) {
        console.warn("No se pudo suscribir al progreso de tareas", error);
        return null;
      }
    };

    let unsubscribe: UnlistenFn | null = null;
    attachJobListener().then((dispose) => {
      unsubscribe = dispose ?? null;
    });

    return () => {
      if (unsubscribe) {
        unsubscribe();
      }
    };
  }, []);

  const loadCandidates = useCallback(
    async (trackId: string) => {
      if (candidateCache[trackId]) {
        setCandidateState({ status: "ready", data: candidateCache[trackId] });
        return;
      }
      setCandidateState({ status: "loading" });
      try {
        const response = await invoke<DiscogsCandidatePayload[]>("list_discogs_candidates", {
          trackId,
        });
        const normalized = response.map(normalizeCandidate);
        setCandidateCache((previous) => ({ ...previous, [trackId]: normalized }));
        setCandidateState({ status: "ready", data: normalized });
      } catch (error) {
        setCandidateState({ status: "error", message: getErrorMessage(error) });
      }
    },
    [candidateCache]
  );

  useEffect(() => {
    if (!selectedTrackId) {
      setCandidateState({ status: "idle" });
      return;
    }
    loadCandidates(selectedTrackId).catch((error) => {
      setCandidateState({ status: "error", message: getErrorMessage(error) });
    });
  }, [selectedTrackId, loadCandidates]);

  const selectedTrack = useMemo(
    () => tracks.find((row) => row.trackId === selectedTrackId) ?? null,
    [tracks, selectedTrackId]
  );

  const hasMore = tracks.length < totalTracks;

  const handleFilterChange = (key: keyof FilterState) => (value: boolean) => {
    setFilters((previous) => ({
      ...previous,
      [key]: value,
    }));
    setCurrentOffset(0);
    setTracks([]);
    setLoadingTracks(true);
    setTrackError(null);
  };

  const handleLoadMore = () => {
    fetchTracks(currentOffset, false).catch((error) => {
      setTrackError(getErrorMessage(error));
    });
  };

  const handleRefreshLikes = async () => {
    try {
      await invoke("refresh_soundcloud_likes");
      setStatusMessage({ type: "success", text: "Actualización de likes solicitada" });
    } catch (error) {
      setStatusMessage({ type: "error", text: getErrorMessage(error) });
    }
  };

  const handleImportRekordbox = async () => {
    try {
      const selected = window.prompt(
        "Introduce la ruta del archivo de biblioteca de Rekordbox",
        ""
      );
      if (!selected) {
        setStatusMessage({ type: "info", text: "Importación cancelada" });
        return;
      }
      await invoke("import_rekordbox_library", { dbPath: selected });
      setStatusMessage({ type: "success", text: "Importación de Rekordbox iniciada" });
    } catch (error) {
      setStatusMessage({ type: "error", text: getErrorMessage(error) });
    }
  };

  const refreshCurrentPage = useCallback(() => {
    fetchTracks(0, true).catch((error) => {
      setTrackError(getErrorMessage(error));
    });
  }, [fetchTracks]);

  const handleConfirmCandidate = async (candidate: DiscogsCandidate) => {
    if (!selectedTrackId || !candidate.releaseId) {
      setStatusMessage({ type: "error", text: "Selecciona un candidato válido." });
      return;
    }
    try {
      await invoke("confirm_discogs_match", {
        trackId: selectedTrackId,
        releaseId: candidate.releaseId,
        candidate: candidate.rawPayload,
      });
      setStatusMessage({ type: "success", text: "Coincidencia confirmada en Discogs" });
      setCandidateCache((previous) => {
        const clone = { ...previous };
        delete clone[selectedTrackId];
        return clone;
      });
      refreshCurrentPage();
    } catch (error) {
      setStatusMessage({ type: "error", text: getErrorMessage(error) });
    }
  };

  const handleIgnoreTrack = async () => {
    if (!selectedTrackId) {
      return;
    }
    try {
      await invoke("ignore_discogs_track", { trackId: selectedTrackId });
      setStatusMessage({ type: "info", text: "Pista marcada como ignorada" });
      setCandidateCache((previous) => {
        const clone = { ...previous };
        delete clone[selectedTrackId];
        return clone;
      });
      refreshCurrentPage();
    } catch (error) {
      setStatusMessage({ type: "error", text: getErrorMessage(error) });
    }
  };

  const handleDownloadTrack = async () => {
    if (!selectedTrackId) {
      return;
    }
    try {
      await invoke("download_track_assets", { trackId: selectedTrackId });
      setStatusMessage({ type: "success", text: "Descarga solicitada" });
    } catch (error) {
      setStatusMessage({ type: "error", text: getErrorMessage(error) });
    }
  };

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <header className="sidebar__header">
          <div>
            <h1>Biblioteca</h1>
            <p className="sidebar__subtitle">
              {tracks.length} de {totalTracks} pistas
            </p>
          </div>
          <div className="sidebar__header-actions">
            <button type="button" className="button button--ghost" onClick={refreshCurrentPage} disabled={loadingTracks}>
              Recargar
            </button>
          </div>
        </header>
        <section className="sidebar__filters">
          <h2>Filtros</h2>
          <div className="filter-group">
            <Checkbox
              id="filter-missing-assets"
              label="Solo faltantes (sin archivo local)"
              checked={filters.missingAssetsOnly}
              onChange={handleFilterChange("missingAssetsOnly")}
            />
            <Checkbox
              id="filter-unresolved-discogs"
              label="Solo sin coincidencia en Discogs"
              checked={filters.unresolvedDiscogsOnly}
              onChange={handleFilterChange("unresolvedDiscogsOnly")}
            />
            <Checkbox
              id="filter-liked"
              label="Solo likes de SoundCloud"
              checked={filters.likedOnly}
              onChange={handleFilterChange("likedOnly")}
            />
            <Checkbox
              id="filter-rekordbox"
              label="Solo en Rekordbox"
              checked={filters.rekordboxOnly}
              onChange={handleFilterChange("rekordboxOnly")}
            />
          </div>
        </section>
        <section className="sidebar__list" aria-live="polite">
          {trackError ? (
            <div className="sidebar__error">{trackError}</div>
          ) : (
            <ul className="track-list">
              {tracks.map((row) => {
                const isSelected = row.trackId === selectedTrackId;
                return (
                  <li
                    key={row.trackId}
                    className={`track-list__item ${isSelected ? "track-list__item--active" : ""}`}
                    onClick={() => setSelectedTrackId(row.trackId)}
                  >
                    <div className="track-list__title">{row.title || "Sin título"}</div>
                    <div className="track-list__meta">{row.artist || "Artista desconocido"}</div>
                    <div className="track-list__badges">
                      {row.liked && <Badge label="Like" variant="primary" />}
                      {row.matched && <Badge label="Discogs" variant="success" />}
                      {row.hasLocalFile ? (
                        <Badge label={row.localAvailable ? "Archivo local" : "Archivo no disponible"} variant={row.localAvailable ? "success" : "warning"} />
                      ) : (
                        <Badge label="Sin archivo" variant="warning" />
                      )}
                      {row.inRekordbox && <Badge label="Rekordbox" variant="neutral" />}
                    </div>
                  </li>
                );
              })}
            </ul>
          )}
          {loadingTracks && <div className="sidebar__loading">Cargando…</div>}
          {hasMore && !loadingTracks && (
            <button type="button" className="button button--ghost sidebar__load-more" onClick={handleLoadMore}>
              Cargar más
            </button>
          )}
        </section>
      </aside>
      <main className="workspace">
        <section className="workspace__toolbar">
          <div className="toolbar__actions">
            <button type="button" className="button" onClick={handleRefreshLikes}>
              Actualizar likes de SoundCloud
            </button>
            <button type="button" className="button" onClick={handleImportRekordbox}>
              Importar biblioteca de Rekordbox
            </button>
          </div>
          {statusMessage && (
            <div className={`toolbar__message toolbar__message--${statusMessage.type}`}>
              {statusMessage.text}
            </div>
          )}
        </section>
        <section className="workspace__content">
          {selectedTrack ? (
            <div className="track-detail">
              <header className="track-detail__header">
                <div>
                  <h2>{selectedTrack.title || "Sin título"}</h2>
                  <p className="track-detail__subtitle">{selectedTrack.artist || "Artista desconocido"}</p>
                </div>
                <div className="track-detail__badges">
                  {selectedTrack.liked && <Badge label="Like" variant="primary" />}
                  {selectedTrack.matched && <Badge label="Discogs" variant="success" />}
                  {selectedTrack.hasLocalFile && (
                    <Badge label={selectedTrack.localAvailable ? "Archivo local" : "Archivo no disponible"} variant={selectedTrack.localAvailable ? "success" : "warning"} />
                  )}
                  {selectedTrack.inRekordbox && <Badge label="Rekordbox" variant="neutral" />}
                </div>
              </header>
              <div className="track-detail__grid">
                <article className="detail-card">
                  <h3>Información general</h3>
                  <dl className="detail-list">
                    <div>
                      <dt>Álbum</dt>
                      <dd>{selectedTrack.album || "Sin datos"}</dd>
                    </div>
                    <div>
                      <dt>Estado Discogs</dt>
                      <dd>{selectedTrack.discogsStatus || "Sin verificar"}</dd>
                    </div>
                    <div>
                      <dt>Confianza</dt>
                      <dd>{selectedTrack.discogsConfidence ? `${selectedTrack.discogsConfidence.toFixed(1)}%` : "N/A"}</dd>
                    </div>
                    <div>
                      <dt>Última comprobación</dt>
                      <dd>{formatDate(selectedTrack.discogsCheckedAt)}</dd>
                    </div>
                    <div>
                      <dt>Mensaje</dt>
                      <dd>{selectedTrack.discogsMessage || "—"}</dd>
                    </div>
                    <div>
                      <dt>Like en SoundCloud</dt>
                      <dd>{formatDate(selectedTrack.soundcloudLikedAt)}</dd>
                    </div>
                    <div>
                      <dt>Enlace SoundCloud</dt>
                      <dd>
                        {selectedTrack.soundcloudPermalinkUrl ? (
                          <a
                            href={selectedTrack.soundcloudPermalinkUrl}
                            target="_blank"
                            rel="noreferrer"
                            className="link"
                          >
                            Abrir en SoundCloud
                          </a>
                        ) : (
                          "Sin enlace"
                        )}
                      </dd>
                    </div>
                    <div>
                      <dt>Archivo local</dt>
                      <dd>{selectedTrack.localLocation || "No registrado"}</dd>
                    </div>
                  </dl>
                </article>
                <article className="detail-card">
                  <h3>Candidatos de Discogs</h3>
                  {candidateState.status === "loading" && <p className="detail-card__empty">Cargando candidatos…</p>}
                  {candidateState.status === "error" && <p className="detail-card__error">{candidateState.message}</p>}
                  {candidateState.status === "ready" && candidateState.data.length === 0 && (
                    <p className="detail-card__empty">No hay candidatos pendientes para esta pista.</p>
                  )}
                  {candidateState.status === "ready" && candidateState.data.length > 0 && (
                    <ul className="candidate-list">
                      {candidateState.data.map((candidate) => (
                        <li key={`${candidate.matchId}-${candidate.releaseId || candidate.title}`} className="candidate-list__item">
                          <div className="candidate-list__main">
                            <div className="candidate-list__info">
                              <h4>{candidate.title || "Sin título"}</h4>
                              <p className="candidate-list__meta">
                                {candidate.year ? `${candidate.year}` : "Año desconocido"}
                                {candidate.country ? ` · ${candidate.country}` : ""}
                                {typeof candidate.score === "number" ? ` · ${candidate.score.toFixed(1)} pts` : ""}
                              </p>
                              {candidate.resourceUrl && (
                                <a className="link" href={candidate.resourceUrl} target="_blank" rel="noreferrer">
                                  Abrir en Discogs
                                </a>
                              )}
                            </div>
                            {candidate.thumb && <img src={candidate.thumb} alt="Carátula" className="candidate-list__thumb" />}
                          </div>
                          <div className="candidate-list__actions">
                            <button
                              type="button"
                              className="button button--small"
                              onClick={() => handleConfirmCandidate(candidate)}
                              disabled={!candidate.releaseId}
                            >
                              Confirmar coincidencia
                            </button>
                          </div>
                        </li>
                      ))}
                    </ul>
                  )}
                </article>
                <article className="detail-card">
                  <h3>Acciones</h3>
                  <div className="detail-actions">
                    <button type="button" className="button button--secondary" onClick={handleDownloadTrack}>
                      Descargar pista
                    </button>
                    <button type="button" className="button button--ghost" onClick={handleIgnoreTrack}>
                      Ignorar pista en Discogs
                    </button>
                  </div>
                </article>
              </div>
            </div>
          ) : (
            <div className="track-detail__empty">Selecciona una pista para ver los detalles.</div>
          )}
        </section>
        <section className="workspace__jobs" aria-live="polite">
          {Object.keys(jobs).length > 0 && (
            <div className="jobs-panel">
              <h3>Tareas en segundo plano</h3>
              <ul className="jobs-list">
                {Object.values(jobs)
                  .sort((a, b) => b.updatedAt - a.updatedAt)
                  .map((job) => {
                    const percentage = job.total ? Math.min(100, Math.round((job.completed / job.total) * 100)) : null;
                    return (
                      <li key={job.id} className={`jobs-list__item jobs-list__item--${job.state}`}>
                        <div className="jobs-list__header">
                          <span className="jobs-list__label">{job.label}</span>
                          <span className="jobs-list__state">{job.state}</span>
                        </div>
                        {percentage !== null && (
                          <div className="jobs-list__progress">
                            <div className="jobs-list__progress-bar" style={{ width: `${percentage}%` }} />
                          </div>
                        )}
                        {job.message && <p className="jobs-list__message">{job.message}</p>}
                      </li>
                    );
                  })}
              </ul>
            </div>
          )}
        </section>
      </main>
    </div>
  );
};

export default App;
