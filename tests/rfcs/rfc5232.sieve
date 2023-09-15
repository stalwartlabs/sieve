
require ["fileinto", "imap4flags", "variables", "relational"];

if size :over 500K {
            setflag "\\Deleted";
}

if header :contains "from" "boss@frobnitzm.example.edu" {
    setflag "flagvar" "\\Flagged";
    fileinto :flags "${flagvar}" "INBOX.From Boss";
}

addflag "flagvar" "\\Deleted";
addflag "flagvar" "\\Answered";

addflag "flagvar" ["\\Deleted", "\\Answered"];

addflag "flagvar" "\\Deleted \\Answered";

if header :contains "Disposition-Notification-To"
    "mel@example.com" {
    addflag "flagvar" "$MDNRequired";
}
if header :contains "from" "imap@cac.washington.example.edu" {
    removeflag "flagvar" "$MDNRequired";
    fileinto :flags "${flagvar}" "INBOX.imap-list";
}

if anyof(hasflag :contains "MyVar" "Junk",
        hasflag :contains "MyVar" ["label", "forward"],
        hasflag :count "ge" :comparator "i;ascii-numeric"
            "MyFlags" "2") {
            fileinto :flags "\\Deleted" "INBOX.From Boss";
            keep :flags "hello";
        }

if size :over 1M
        {
        addflag "MyFlags" "Big";
        if header :is "From" "boss@company.example.com"
                    {
# The message will be marked as "\Flagged Big" when filed into
# mailbox "Big messages"
                    addflag "MyFlags" "\\Flagged";
                    }
        fileinto :flags "${MyFlags}" "Big messages";
        }

if header :is "From" "grandma@example.net"
        {
        addflag "MyFlags" ["\\Answered", "$MDNSent"];
# If the message is bigger than 1Mb it will be marked as
# "Big \Answered $MDNSent" when filed into mailbox "grandma".
# If the message is shorter than 1Mb it will be marked as
# "\Answered $MDNSent"
        fileinto :flags "${MyFlags}" "GrandMa";
        }

#
# Handle messages from known mailing lists
# Move messages from IETF filter discussion list to filter folder
#
if header :is "Sender" "owner-ietf-mta-filters@example.org"
        {
        set "MyFlags" "\\Flagged $Work";
# Message will have both "\Flagged" and $Work flags
        keep :flags "${MyFlags}";
        }

#
# Keep all messages to or from people in my company
#
elsif address :domain :is ["From", "To"] "company.example.com"
        {
        keep :flags "${MyFlags}"; # keep in "Inbox" folder
        }

# Try to catch unsolicited email.  If a message is not to me,
# or it contains a subject known to be spam, file it away.
#
elsif anyof (not address :all :contains
                ["To", "Cc"] "me@company.example.com",
            header :matches "subject"
                ["*make*money*fast*", "*university*dipl*mas*"])
            {
        removeflag "MyFlags" "\\Flagged";
        fileinto :flags "${MyFlags}" "spam";
        }
else
        {
        # Move all other external mail to "personal"
        # folder.
        fileinto :flags "${MyFlags}" "personal";
}
