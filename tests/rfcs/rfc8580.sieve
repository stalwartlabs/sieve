require ["vacation", "fcc", "mailbox", "special-use", "imap4flags", "enotify"];

vacation :days 7
        :from "hemingway@example.com" 
        :specialuse "\\Sent" :create
        :fcc "INBOX.Sent" :flags ["\\Seen"] "Gone Fishin'";

notify :fcc "INBOX.Sent"
        :message "You got mail!"
        "mailto:ken@example.com";

if notify_method_capability "xmpp:" "fcc" "yes" {
    notify :fcc "INBOX.Sent"
            :message "You got mail"
            "xmpp:ken@example.com?message;subject=SIEVE";
} else {
    notify :fcc "INBOX.Sent"
            :message "You got mail!"
            "mailto:ken@example.com";
}

