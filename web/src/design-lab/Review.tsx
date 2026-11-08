// The lab's front page: the one open decision, alone and large, then a few quiet links.
// Clicking a picture opens a full-screen viewer in the page. The arrow keys move between the
// options and Escape closes it. The archive page shows earlier decisions, one titled group each.
import { useEffect, useRef, useState } from 'react';
import { Link } from 'react-router';
import { useTranslation } from 'react-i18next';
import { usePageTitle } from '../hooks';
import { HISTORY, OPEN, PROVISIONAL, type Decision } from './decisions';

const IMAGES = import.meta.glob('./mockups/*.webp', { eager: true, import: 'default', query: '?url' }) as Record<string, string>;

function image(name: string): string {
  return IMAGES[`./mockups/${name}.webp`] ?? '';
}

const LETTERS = 'ABCDEFGH';

interface Slide {
  src: string;
  /** For example "A. Overview cards". */
  title: string;
  summary?: string;
}

/** A full-screen picture viewer on the native dialog element. */
function Viewer({ slides, index, onIndex, onClose }: { slides: Slide[]; index: number | null; onIndex: (index: number) => void; onClose: () => void }) {
  const { t } = useTranslation();
  const ref = useRef<HTMLDialogElement>(null);
  const open = index !== null;

  useEffect(() => {
    const dialog = ref.current;
    if (!dialog) return;
    if (open && !dialog.open) dialog.showModal();
    if (!open && dialog.open) dialog.close();
  }, [open]);

  useEffect(() => {
    if (index === null) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'ArrowRight') onIndex((index + 1) % slides.length);
      if (event.key === 'ArrowLeft') onIndex((index - 1 + slides.length) % slides.length);
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [index, slides.length, onIndex]);

  const slide = index === null ? null : slides[index];
  return (
    <dialog ref={ref} className="viewer" aria-label={slide?.title ?? ''} onClose={onClose}>
      {slide && index !== null ? (
        <>
          <div className="viewer-bar">
            <p className="viewer-title">
              <strong>{slide.title}</strong>
              {slide.summary ? <span> {slide.summary}</span> : null}
            </p>
            <p className="viewer-count">{t('designLab.review.count', { n: index + 1, total: slides.length })}</p>
            <button type="button" className="viewer-close" onClick={onClose}>
              {t('designLab.review.close')}
            </button>
          </div>
          <div className="viewer-stage">
            {slides.length > 1 ? (
              <button
                type="button"
                className="viewer-nav viewer-prev"
                aria-label={t('designLab.review.previous')}
                onClick={() => onIndex((index - 1 + slides.length) % slides.length)}
              >
                <span aria-hidden="true">‹</span>
              </button>
            ) : null}
            <img src={slide.src} alt={slide.title} />
            {slides.length > 1 ? (
              <button
                type="button"
                className="viewer-nav viewer-next"
                aria-label={t('designLab.review.next')}
                onClick={() => onIndex((index + 1) % slides.length)}
              >
                <span aria-hidden="true">›</span>
              </button>
            ) : null}
          </div>
          <p className="viewer-hint">{t('designLab.review.keys')}</p>
        </>
      ) : null}
    </dialog>
  );
}

function slidesFor(decision: Decision, t: (key: string) => string, withNames: boolean): Slide[] {
  return decision.options.map((name, index) => ({
    src: image(name),
    title: withNames
      ? `${LETTERS[index]}. ${t(`designLab.review.${decision.id}.options.${index}.name`)}`
      : `${t(`designLab.review.${decision.id}.title`)} ${index + 1}`,
    summary: withNames ? t(`designLab.review.${decision.id}.options.${index}.summary`) : undefined,
  }));
}

export function ReviewPage() {
  const { t } = useTranslation();
  usePageTitle(t('designLab.title'));
  const [viewing, setViewing] = useState<number | null>(null);
  const slides = OPEN ? slidesFor(OPEN, t, true) : [];

  return (
    <div className="review">
      <header className="review-head">
        <p className="review-kicker">{t('designLab.review.kicker')}</p>
        {OPEN ? (
          <>
            <h1>{t(`designLab.review.${OPEN.id}.title`)}</h1>
            <p className="review-question">{t(`designLab.review.${OPEN.id}.question`)}</p>
            <p className="review-how">{t('designLab.review.how')}</p>
          </>
        ) : PROVISIONAL.length > 0 ? null : (
          <h1>{t('designLab.review.nothing')}</h1>
        )}
      </header>
      {OPEN ? (
        <ul className="review-options">
          {slides.map((slide, index) => (
            <li key={slide.src} className="review-option">
              <button type="button" className="review-shot" onClick={() => setViewing(index)}>
                <img src={slide.src} alt={slide.title} />
              </button>
              <div className="review-text">
                <h3>
                  <span className="review-letter">{LETTERS[index]}</span> {slide.title.slice(3)}
                </h3>
                <p>{slide.summary}</p>
              </div>
            </li>
          ))}
        </ul>
      ) : null}
      <Viewer slides={slides} index={viewing} onIndex={setViewing} onClose={() => setViewing(null)} />
      {!OPEN && PROVISIONAL.length > 0 ? <Provisional /> : null}
      <nav className="review-links" aria-label={t('designLab.review.more')}>
        <Link to="history">{t('designLab.review.historyLink')}</Link>
        <Link to="components">{t('designLab.components.link')}</Link>
        <Link to="screens">{t('designLab.screens.link')}</Link>
      </nav>
    </div>
  );
}

/** The overnight choices, each with its options and the picked one marked. */
function Provisional() {
  const { t } = useTranslation();
  const [viewing, setViewing] = useState<{ group: number; index: number } | null>(null);
  const groups = PROVISIONAL.map((decision) => slidesFor(decision, t, true));
  return (
    <>
      <div className="review-head">
        <p className="review-kicker">{t('designLab.review.provisional.kicker')}</p>
        <h1>{t('designLab.review.provisional.title')}</h1>
        <p className="review-question">{t('designLab.review.provisional.body')}</p>
        <p className="review-how">{t('designLab.review.how')}</p>
      </div>
      {PROVISIONAL.map((decision, group) => (
        <section key={decision.id} className="review-group" aria-labelledby={`provisional-${decision.id}`}>
          <h2 id={`provisional-${decision.id}`}>{t(`designLab.review.${decision.id}.title`)}</h2>
          <p className="review-question">{t(`designLab.review.${decision.id}.question`)}</p>
          <p className="review-how">
            <strong>{t('designLab.review.provisional.why')}:</strong> {t(`designLab.review.${decision.id}.why`)}
          </p>
          <ul className="review-options">
            {(groups[group] ?? []).map((slide, index) => (
              <li key={slide.src} className={`review-option${index === decision.chosen ? ' is-picked' : ''}`}>
                <button type="button" className="review-shot" onClick={() => setViewing({ group, index })}>
                  <img src={slide.src} alt={slide.title} />
                </button>
                <div className="review-text">
                  <h3>
                    <span className="review-letter">{LETTERS[index]}</span> {slide.title.slice(3)}
                    {index === decision.chosen ? <span className="review-picked">{t('designLab.review.provisional.picked')}</span> : null}
                  </h3>
                  <p>{slide.summary}</p>
                </div>
              </li>
            ))}
          </ul>
        </section>
      ))}
      <Viewer
        slides={viewing ? (groups[viewing.group] ?? []) : []}
        index={viewing ? viewing.index : null}
        onIndex={(index) => setViewing((current) => (current ? { ...current, index } : current))}
        onClose={() => setViewing(null)}
      />
    </>
  );
}

export function HistoryPage() {
  const { t } = useTranslation();
  usePageTitle(t('designLab.review.historyTitle'));
  const [viewing, setViewing] = useState<{ group: number; index: number } | null>(null);
  const groupSlides = HISTORY.map((decision) => slidesFor(decision, t, false));

  return (
    <div className="review">
      <header className="review-head">
        <p>
          <Link to="/design-lab">{t('designLab.review.back')}</Link>
        </p>
        <h1>{t('designLab.review.historyTitle')}</h1>
      </header>
      {HISTORY.map((decision, group) => (
        <section key={decision.id} className="review-group" aria-labelledby={`review-${decision.id}`}>
          <h2 id={`review-${decision.id}`}>{t(`designLab.review.${decision.id}.title`)}</h2>
          <p className="review-question">{t(`designLab.review.${decision.id}.outcome`)}</p>
          <ul className="review-thumbs">
            {(groupSlides[group] ?? []).map((slide, index) => (
              <li key={slide.src}>
                <button type="button" className="review-shot" onClick={() => setViewing({ group, index })}>
                  <img src={slide.src} alt={slide.title} loading="lazy" />
                </button>
              </li>
            ))}
          </ul>
        </section>
      ))}
      <Viewer
        slides={viewing ? (groupSlides[viewing.group] ?? []) : []}
        index={viewing ? viewing.index : null}
        onIndex={(index) => setViewing((current) => (current ? { ...current, index } : current))}
        onClose={() => setViewing(null)}
      />
    </div>
  );
}
