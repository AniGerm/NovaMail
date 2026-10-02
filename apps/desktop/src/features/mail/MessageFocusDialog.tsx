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
}

/** Full-size modal reading surface (same chrome pattern as Settings). */
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
  const description = message
    ? `${message.summary.from.name?.trim() || message.summary.from.email}`
    : undefined;

  return (
    <Dialog
      open={open}
      onClose={onClose}
      title={title}
      description={description}
      className="h-[min(92vh,920px)] w-[min(1100px,96vw)] max-w-none"
    >
      <div className="min-h-0 flex-1 overflow-y-auto">
        <ReadingPane
          message={message}
          aiEnabled={aiEnabled}
          inSpamFolder={inSpamFolder}
          focusMode
          {...paneProps}
        />
      </div>
    </Dialog>
  );
}
