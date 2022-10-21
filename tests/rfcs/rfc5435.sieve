require ["enotify", "fileinto", "variables", "envelope"];

if not valid_notify_method ["mailto:",
        "http://gw.example.net/notify?test"] {
    stop;
}

if notify_method_capability
    "xmpp:tim@example.com?message;subject=SIEVE"
    "Online"
    "yes" {
    notify :importance "1" :message "You got mail"
        "xmpp:tim@example.com?message;subject=SIEVE";
} else {
    notify :message "You got mail" "tel:+14085551212";
}

if header :contains "from" "boss@example.org" {
    notify :importance "1"
        :message "This is probably very important"
                    "mailto:alm@example.com";
    # Don't send any further notifications
    stop;
}

if header :contains "to" "sievemailinglist@example.org" {
    # :matches is used to get the value of the Subject header
    if header :matches "Subject" "*" {
        set "subject" "${1}";
    }

    # :matches is used to get the value of the From header
    if header :matches "From" "*" {
        set "from" "${1}";
    }

    notify :importance "3"
        :message "[SIEVE] ${from}: ${subject}"
        "mailto:alm@example.com";
    fileinto "INBOX.sieve";
}

if header :matches "from" "*@*.example.org" {
    # :matches is used to get the MAIL FROM address
    if envelope :all :matches "from" "*" {
        set "env_from" " [really: ${1}]";
    }

    # :matches is used to get the value of the Subject header
    if header :matches "Subject" "*" {
        set "subject" "${1}";
    }

    # :matches is used to get the address from the From header
    if address :matches :all "from" "*" {
        set "from_addr" "${1}";
    }
notify :message "${from_addr}${env_from}: ${subject}"
                    "mailto:alm@example.com";
}

set "notif_method"
"xmpp:tim@example.com?message;subject=SIEVE;body=You%20got%20mail";

if header :contains "subject" "Your dog" {
    set "notif_method" "tel:+14085551212";
}

if header :contains "to" "sievemailinglist@example.org" {
    set "notif_method" "";
}

if not string :is "${notif_method}" "" {
    notify "${notif_method}";
}

if header :contains "from" "boss@example.org" {
    # :matches is used to get the value of the Subject header
    if header :matches "Subject" "*" {
        set "subject" "${1}";
    }

    # don't need high importance notification for
    # a 'for your information'
    if not header :contains "subject" "FYI:" {
        notify :importance "1" :message "BOSS: ${subject}"
                        "tel:+14085551212";
    }
}
