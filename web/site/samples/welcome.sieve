# Welcome to Sievepad!
#
# Sieve (RFC 5228) is the standard language for filtering email on the
# mail server. This script runs in your browser on the same interpreter
# that powers Stalwart Mail Server (https://stalw.art).
#
# Press Run (Ctrl+Enter or Cmd+Enter) to filter the message in the middle
# column, then open the Messages tab on the right to see what changed.
# Hover over any command to read its documentation.
#
# Everything runs locally in your browser: scripts and messages are saved
# in this browser only and are never sent to or stored on a server.

require ["envelope", "fileinto", "mailbox", "special-use", "imap4flags",
         "variables", "relational", "comparator-i;ascii-numeric",
         "spamtest", "spamtestplus", "date", "duplicate", "extlists",
         "editheader", "mime", "foreverypart", "extracttext", "replace",
         "vacation"];

# Drop the message if the very same one was delivered in the last hour.
# Press "Run again" to simulate a second delivery.
if duplicate :seconds 3600 {
    discard;
    stop;
}

# Anything the spam filter scored 80% or higher goes to Junk.
if spamtest :percent :value "ge" :comparator "i;ascii-numeric" "80" {
    fileinto :specialuse "\\Junk" :create "Junk";
    stop;
}

# Mailing lists get a folder of their own, such as "Lists/rust-users".
if header :matches "List-Id" "*<*.*>*" {
    set :lower "list" "${2}";
    fileinto :create "Lists/${list}";
    stop;
}

# Capture a few details about the message with variables and wildcard
# matches. Variables first assigned inside a block only live in that block,
# so declare the ones used later at the top level.
set "domain" "unknown";
set "subject" "";
set "sender" "there";
if address :domain :matches "from" "*" {
    set :lower "domain" "${1}";
}
if header :matches "Subject" "*" {
    set "subject" "${1}";
}
if header :matches "From" "* <*>" {
    set "sender" "${1}";
}

# People in your address book are flagged as important.
if address :all :list "from" "addrbook:default" {
    addflag "\\Flagged";
}

# Tag messages that carry a PDF anywhere in their MIME structure.
if header :mime :anychild :contenttype "Content-Type" "application/pdf" {
    addflag "$HasPDF";
}

# Note when the message arrived outside office hours.
if anyof (currentdate :value "lt" :comparator "i;ascii-numeric" "hour" "08",
          currentdate :value "ge" :comparator "i;ascii-numeric" "hour" "18") {
    addheader "X-After-Hours" "yes";
}

# Stamp the message so you can see it went through the filter.
addheader :last "X-Sieve-Filter" "Sievepad by Stalwart Labs (https://stalw.art)";

# Append a footer to every plain text part.
foreverypart {
    if header :mime :subtype "Content-Type" "plain" {
        extracttext "body";
        replace text:
${body}
--
Regain control of your inbox with Stalwart: https://stalw.art
.
;
    }
}

# Let the sender know you are away, at most once a week per sender.
vacation :days 7 :subject "Away: ${subject}" text:
Hi ${sender},

Thanks for your message. I am out of the office this week and will
get back to you as soon as I return.

--
This reply was sent by a Sieve script running on Stalwart Mail Server.
Regain control of your inbox: https://stalw.art
.
;

# Important mail stays in the inbox, the rest is sorted by domain.
if hasflag "\\Flagged" {
    keep;
} else {
    fileinto :create "Contacts/${domain}";
}
