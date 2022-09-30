require "fileinto";
require "special-use";

fileinto :specialuse "\\Archive" "INBOX/Archive";

fileinto :specialuse "\\Junk" "Spam";

fileinto :specialuse "\\Junk" :create "Spam";

if environment :contains "imap.mailbox" "*" {
    set "mailbox" "${1}";
}

if allof(
    environment "imap.cause" "COPY",
    specialuse_exists "${mailbox}" "\\Junk") {
    redirect "spam-report@example.org";
}

