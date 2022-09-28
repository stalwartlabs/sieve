if size :over 100k { # this is a comment
    discard;
}

if size :over 100K { /* this is a comment
    this is still a comment */ discard /* this is a comment
    */ ;
}

require "encoded-character";
if header :contains "Subject" "$${hex:24 24}" {
    discard;
}

if anyof (not exists ["From", "Date"],
    header :contains "from" "fool@example.com") {
    discard;
}

if header :contains :comparator "i;octet" "Subject"
    "MAKE MONEY FAST" {
    discard;
}

if size :over 500K { discard; }

require "fileinto";
if header :contains "from" "coyote" {
    discard;
} elsif header :contains ["subject"] ["$$$"] {
    discard;
} else {
    fileinto "INBOX";
}

if header :contains ["From"] ["coyote"] {
    redirect "acm@example.com";
} elsif header :contains "Subject" "$$$" {
    redirect "postmaster@example.com";
} else {
    redirect "field@example.com";
}

require "fileinto";
if header :contains ["from"] "coyote" {
    fileinto "INBOX.harassment";
}

redirect "bart@example.com";

if size :under 1M { keep; } else { discard; }

if header :contains ["from"] ["idiot@example.com"] {
    discard;
}

if address :is :all "from" "tim@example.com" {
    discard;
}

require "envelope";
if envelope :all :is "from" "tim@example.com" {
    discard;
}

if not exists ["From","Date"] {
    discard;
}

if anyof(header :is ["X-Caffeine"] [""], header :contains ["X-Caffeine"] [""], not header :matches "Cc" "?*") {
    stop;
}

#
# Example Sieve Filter
# Declare any optional features or extension used by the script
#
require ["fileinto"];

#
# Handle messages from known mailing lists
# Move messages from IETF filter discussion list to filter mailbox
#
if header :is "Sender" "owner-ietf-mta-filters@imc.org"
        {
        fileinto "filter";  # move to "filter" mailbox
        }
#
# Keep all messages to or from people in my company
#
elsif address :DOMAIN :is ["From", "To"] "example.com"
        {
        keep;               # keep in "In" mailbox
        }

#
# Try and catch unsolicited email.  If a message is not to me,
# or it contains a subject known to be spam, file it away.
#
elsif anyof (NOT address :all :contains
                ["To", "Cc", "Bcc"] "me@example.com",
                header :matches "subject"
                ["*make*money*fast*", "*university*dipl*mas*"])
        {
        fileinto "spam";   # move to "spam" mailbox
        }
else
        {
        # Move all other (non-company) mail to "personal"
        # mailbox.
        fileinto "personal";
        }
