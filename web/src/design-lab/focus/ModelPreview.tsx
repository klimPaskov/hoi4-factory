// The live game preview wired to the editor's local state: selecting, dragging, applying and
// discarding on the picture all go through the same model as the list and the inspector.
import { GamePreview } from './GamePreview';
import type { FocusModel } from './useFocusModel';

export function ModelPreview({ model, label, className }: { model: FocusModel; label: string; className?: string }) {
  return (
    <GamePreview
      image={model.render}
      nodes={model.nodes}
      diagnostics={model.diagnostics}
      selectedId={model.selectedId}
      onSelect={model.select}
      query={model.query}
      redrawing={model.redrawing}
      readOnly={model.refreshing}
      onMove={model.move}
      onApply={model.apply}
      onDiscard={model.discard}
      revealTick={model.revealTick}
      revealId={model.selectedId}
      label={label}
      className={className}
    />
  );
}
