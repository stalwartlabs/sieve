require ["duplicate", "variables", "fileinto", "mailbox", "imap4flags", "envelope", "enotify"];

if duplicate {
    discard;
}

if duplicate :header "message-id" {
    discard;
}

if header :matches "message-id" "*" {
    if duplicate :uniqueid "${0}" {
    discard;
    }
}

if duplicate {
    fileinto :create "Trash/Duplicate";
}

if header :matches "subject" "ALERT: *" {
    if duplicate :seconds 60 :uniqueid "${1}" {
    setflag "\\seen";
    }
    fileinto "Alerts";
}

if envelope :matches "from" "*" { set "sender" "${1}"; }
if header :matches "subject" "*" { set "subject" "${1}"; }

if not duplicate :seconds 1800 :uniqueid "${sender}"
{
    notify :message "[SIEVE] ${sender}: ${subject}"
    "xmpp:user@im.example.com";
}

if envelope :matches "from" "*" { set "sender" "${1}"; }
if header :matches "subject" "*" { set "subject" "${1}"; }

# account for 'Re:' prefix
if string :comparator "i;ascii-casemap"
    :matches "${subject}" "Re:*"
{
    set "subject" "${1}";
}
if not duplicate :seconds 1800
    :uniqueid "${sender} ${subject}"
{
    notify :message "[SIEVE] ${sender}: ${subject}"
    "xmpp:user@im.example.com";
}

if duplicate :header "X-Event-ID" :handle "notifier" {
    discard;
}
if allof (
    duplicate :header "X-Ticket-ID" :handle "support",
    address "to" "support@example.com",
    header :contains "subject" "fileserver")
{
    setflag "\\seen";
}
