// The guide mascot: a brick factory with a scarf, made with the image model (one image per pose,
// docs/design/DESIGN-DECISIONS.md) and moved with CSS. It points people at features in the
// tutorial. The images are decorative, so whatever the mascot "says" is real text beside it.
import type { ReactNode } from 'react';

const POSES = import.meta.glob('../assets/mascot/*.webp', { eager: true, import: 'default', query: '?url' }) as Record<
  string,
  string
>;

export type MascotPose =
  | 'master'
  | 'wave'
  | 'point-right'
  | 'point-left'
  | 'point-up'
  | 'point-down'
  | 'cheer'
  | 'think'
  | 'thumbs-up'
  | 'sorry'
  | 'surprised'
  | 'working'
  | 'reading'
  | 'waiting'
  | 'peek';

export function mascotUrl(pose: MascotPose): string {
  return POSES[`../assets/mascot/${pose}.webp`] ?? '';
}

/**
 * The mascot in one pose. `motion` picks the CSS movement: `enter` hops in once, `idle` bobs
 * gently, `bounce` hops in place (for drawing attention). Reduced motion keeps it still.
 * A changed `pose` remounts the image, so each new pose hops in.
 */
export function Mascot({
  pose,
  size = 'md',
  motion = 'idle',
  children,
}: {
  pose: MascotPose;
  size?: 'sm' | 'md' | 'lg';
  motion?: 'enter' | 'idle' | 'bounce' | 'none';
  /** What the mascot says, shown in a speech bubble. */
  children?: ReactNode;
}) {
  return (
    <div className={`mascot mascot-${size}`}>
      <img key={pose} className={`mascot-img mascot-${motion}`} src={mascotUrl(pose)} alt="" draggable={false} />
      {children ? <div className="mascot-bubble">{children}</div> : null}
    </div>
  );
}
