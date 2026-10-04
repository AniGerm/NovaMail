import { Dialog } from "@novamail/ui";

import { ReadingPane } from "@/features/mail/ReadingPane";
import type {
  EventSuggestionDto,
  MessageDetailDto,
  SnoozePreset,
} from "@/shared/api/types";
import { useT } from "@/shared/i18n/useT";

interface MessageFocusDialogProps {
  open: boolean;
  message?: MessageDetailDto | null;
  aiEnabled?: boolean;
  inSpamFolder?: boolean;
  onClose: () => void;
  onReply: () => void;
  onForward?: () => void;
  onToggleStar: () => void;
  onDelete?: () => void;
  onArchive?: () => void;
  onMarkSpam?: () => void;
  onMarkNotSpam?: () => void;
  onSnooze?: (preset: SnoozePreset) => void;
  onCreateEvent?: (suggestion?: EventSuggestionDto) => void;
  onCreateTask?: () => void;
  onReplySent?: () => void;
  onMarkRead?: () => void;
}

/** Near-fullscreen reading surface with equal inset on all sides. */
export function MessageFocusDialog({
  open,
  message,
  aiEnabled,
  inSpamFolder,
  onClose,
  ...paneProps
}: MessageFocusDialogProps) {
  const t = useT();
  const title = message?.summary.subject?.trim() || t("noSubject");

  return (
    <Dialog
      open={open}
      onClose={onClose}
      title={title}
      dense
      className="h-full w-full max-w-none p-3"
    >
      <div className="min-h-0 flex-1 overflow-y-auto">
        <ReadingPane
          message={message}
          aiEnabled={aiEnabled}
          inSpamFolder={inSpamFolder}
          focusMode
          {...paneProps}
          onToggleFocus={onClose}
        />
      </div>
    </Dialog>
  );
}
