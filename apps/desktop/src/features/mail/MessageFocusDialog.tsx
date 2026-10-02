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
  const description = message
    ? `${message.summary.from.name?.trim() || message.summary.from.email}`
    : undefined;

  return (
    <Dialog
      open={open}
      onClose={onClose}
      title={title}
      description={description}
      className="h-[calc(100vh-2rem)] w-[calc(100vw-2rem)] max-w-none p-4 sm:p-5"
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
