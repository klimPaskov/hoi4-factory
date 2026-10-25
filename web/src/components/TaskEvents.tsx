import { useCallback, useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { eventStreamUrl, listEvents, type RunEvent } from '../api/tasks';
import { formatDateTime } from '../format';
import { Alert } from './ui';

const PAGE = 200;
const POLL_MS = 3000;

type Mode = 'stream' | 'polling';

/**
 * The live event log of a task. Events arrive over EventSource and are deduplicated by `seq`.
 * A seq that is not exactly last + 1 means events were missed, so the gap is filled from
 * `/events?after=<last>` (C-03). If the stream fails, the log polls every 3 seconds instead.
 */
export function useTaskEvents(projectId: string, taskId: string, onNew?: () => void) {
  const [events, setEvents] = useState<RunEvent[]>([]);
  const [mode, setMode] = useState<Mode>('stream');
  const lastRef = useRef(0);
  const flightRef = useRef<Promise<void> | null>(null);
  const againRef = useRef(false);
  const liveRef = useRef(false);
  const onNewRef = useRef(onNew);
  useEffect(() => {
    onNewRef.current = onNew;
  });

  const add = useCallback((incoming: RunEvent[]) => {
    const fresh = incoming.filter((e) => e.seq > lastRef.current);
    if (fresh.length === 0) return;
    fresh.sort((a, b) => a.seq - b.seq);
    const accepted: RunEvent[] = [];
    for (const event of fresh) {
      // Only an unbroken run is accepted; anything after a hole waits for the gap fill.
      if (event.seq !== lastRef.current + 1) break;
      accepted.push(event);
      lastRef.current = event.seq;
    }
    if (accepted.length === 0) return;
    setEvents((current) => [...current, ...accepted]);
    // The history loaded on open is not news: the task was read at the same moment.
    if (liveRef.current) onNewRef.current?.();
  }, []);

  // Fetch everything after the last seq seen. One request chain at a time.
  const catchUp = useCallback((): Promise<void> => {
    if (flightRef.current) {
      againRef.current = true;
      return flightRef.current;
    }
    const run = (async () => {
      try {
        do {
          againRef.current = false;
          for (;;) {
            const before = lastRef.current;
            const page = await listEvents(projectId, taskId, before, PAGE);
            add(page.events);
            if (page.events.length < PAGE || lastRef.current === before) break;
          }
        } while (againRef.current);
      } catch {
        // A failed fill is retried by the next event or poll.
      } finally {
        flightRef.current = null;
      }
    })();
    flightRef.current = run;
    return run;
  }, [projectId, taskId, add]);

  useEffect(() => {
    let closed = false;
    let source: EventSource | null = null;
    let timer: ReturnType<typeof setInterval> | null = null;

    const startPolling = () => {
      if (closed || timer) return;
      setMode('polling');
      timer = setInterval(() => void catchUp(), POLL_MS);
    };

    void catchUp().then(() => {
      if (closed) return;
      liveRef.current = true;
      if (typeof EventSource === 'undefined') {
        startPolling();
        return;
      }
      source = new EventSource(eventStreamUrl(projectId, taskId, lastRef.current));
      source.addEventListener('run_event', (message) => {
        let event: RunEvent;
        try {
          event = JSON.parse((message as MessageEvent<string>).data) as RunEvent;
        } catch {
          return;
        }
        if (event.seq <= lastRef.current) return;
        if (event.seq === lastRef.current + 1) add([event]);
        else void catchUp();
      });
      source.onerror = () => {
        source?.close();
        source = null;
        startPolling();
      };
    });

    return () => {
      closed = true;
      source?.close();
      if (timer) clearInterval(timer);
    };
  }, [projectId, taskId, add, catchUp]);

  return { events, mode };
}

export function TaskEvents({
  projectId,
  taskId,
  onNew,
}: {
  projectId: string;
  taskId: string;
  onNew?: () => void;
}) {
  const { t } = useTranslation();
  const { events, mode } = useTaskEvents(projectId, taskId, onNew);

  return (
    <section className="card" aria-labelledby="task-events-title">
      <h2 id="task-events-title">{t('tasks.eventsTitle')}</h2>
      <Alert kind="info">{mode === 'stream' ? t('tasks.eventsLive') : t('tasks.eventsPolling')}</Alert>
      {events.length === 0 ? (
        <p className="muted">{t('tasks.eventsEmpty')}</p>
      ) : (
        <ol className="project-list" aria-labelledby="task-events-title">
          {events.map((event) => (
            <li key={event.seq}>
              <p>
                <strong>{event.seq}</strong> <span className="muted small">{formatDateTime(event.created_at)}</span>{' '}
                <code>{event.type}</code> <bdi dir="auto">{event.summary}</bdi>
              </p>
              <details>
                <summary>{t('tasks.eventsDetails')}</summary>
                <pre className="excerpt" dir="ltr">
                  {JSON.stringify(event.payload, null, 2)}
                </pre>
              </details>
            </li>
          ))}
        </ol>
      )}
    </section>
  );
}
