// Candidate logo marks, one per direction. Decorative: the brand link next to each carries the
// name. Colors come from CSS classes so each theme can recolor them.

const GEAR_TEETH = [0, 45, 90, 135, 180, 225, 270, 315];

export function WorkbenchMark() {
  return (
    <svg className="mark" viewBox="0 0 32 32" aria-hidden="true" focusable="false">
      <rect className="mark-bg" width="32" height="32" rx="8" />
      <path className="mark-fg" d="M6.5 24.5V15l5-4v4l5-4v4l5-4V7.5h4v17z" />
    </svg>
  );
}

export function DossierMark() {
  return (
    <svg className="mark" viewBox="0 0 32 32" aria-hidden="true" focusable="false">
      <circle className="mark-ring" cx="16" cy="16" r="14.5" />
      <circle className="mark-ring mark-ring-thin" cx="16" cy="16" r="11.5" />
      <g className="mark-fg">
        <circle cx="16" cy="16" r="5" />
        {GEAR_TEETH.map((angle) => (
          <rect key={angle} x="14.6" y="8.2" width="2.8" height="3.6" rx="0.5" transform={`rotate(${angle} 16 16)`} />
        ))}
      </g>
      <circle className="mark-hole" cx="16" cy="16" r="2" />
    </svg>
  );
}

export function WarRoomMark() {
  return (
    <svg className="mark" viewBox="0 0 32 32" aria-hidden="true" focusable="false">
      <path className="mark-ring" d="M16 2.5 27.7 9.25v13.5L16 29.5 4.3 22.75V9.25z" />
      <path className="mark-fg" d="M9.5 21 16 15.4l6.5 5.6v3.6L16 19l-6.5 5.6z" />
      <path className="mark-fg mark-accent" d="M9.5 14.6 16 9l6.5 5.6v3.6L16 12.6l-6.5 5.6z" />
    </svg>
  );
}

export function StudioMark() {
  return (
    <svg className="mark" viewBox="0 0 32 32" aria-hidden="true" focusable="false">
      <rect className="mark-bg" width="32" height="32" rx="11" />
      <path className="mark-fg" d="M7 25v-8.6c0-.6.3-1.1.8-1.4l3.7-2.5v3.2l4.3-2.9v3.2l4.3-2.9V10a1 1 0 0 1 1-1h2.4a1 1 0 0 1 1 1v15z" />
      <circle className="mark-fg mark-puff" cx="22.6" cy="5.9" r="1.7" />
      <circle className="mark-fg mark-puff" cx="25.8" cy="3.9" r="1.1" />
    </svg>
  );
}
