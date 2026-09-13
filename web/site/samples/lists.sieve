# Sort mailing lists into folders
#
# Uses "regex" to pull the list name out of the List-Id header and
# "variables" to build the folder name. Messages that mention a keyword
# in the body are flagged, and replies to your own posts are marked read.

require ["fileinto", "mailbox", "imap4flags", "variables", "regex", "body",
         "subaddress", "envelope"];

if header :regex "List-Id" "<([a-z0-9-]+)\\.([a-z0-9.-]+)>" {
    set :lower "list" "${1}";
    set :lower "host" "${2}";

    if body :text :contains ["release", "security advisory"] {
        addflag "\\Flagged";
    }

    if header :contains "In-Reply-To" "@example.org>" {
        addflag "\\Seen";
    }

    fileinto :create "Lists/${host}/${list}";
    stop;
}

# Plus addressing: mail sent to jane+shopping@example.org goes to "shopping".
if envelope :detail :matches "to" "*" {
    if string :is "${1}" "" {
        keep;
    } else {
        set :lower "folder" "${1}";
        fileinto :create "${folder}";
    }
}
