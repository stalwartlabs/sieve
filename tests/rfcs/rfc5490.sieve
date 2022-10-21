require ["fileinto", "reject", "mailbox", "mboxmetadata", "vacation", 
"servermetadata", "variables", "envelope", "enotify"];
if mailboxexists "Partners" {
    fileinto :create "Partners";
} else {
    reject "This message was not accepted by the Mailstore";
}

if metadata :is "INBOX"
    "/private/vendor/vendor.isode/auto-replies" "on" {

    vacation text:
I'm away on holidays till March 2009.
Expect a delay.
.
;
}

if servermetadata :matches
    "/private/vendor/vendor.isode/notification-uri" "*" {
    set "notif_uri" "${0}";
}

if not string :is "${notif_uri}" "none" {
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
            "${notif_uri}";
}

if servermetadataexists ["hello", "world"] {
    stop;
}

if metadataexists "INBOX" ["hi", "there"] {
    discard;
}


