# Out of office with dates
#
# Replies only while the current date is inside the vacation window,
# never to mailing lists or automated senders, and at most once every
# three days per sender. Change the dates to see the reply disappear.

require ["vacation", "vacation-seconds", "date", "relational", "variables",
         "envelope"];

if currentdate :value "ge" "date" "2026-01-01" {
    if currentdate :value "le" "date" "2026-12-31" {
        if not anyof (exists "List-Id",
                      header :contains "Precedence" ["bulk", "list", "junk"],
                      header :is "Auto-Submitted" ["auto-replied", "auto-generated"],
                      envelope :localpart :is "from" ["mailer-daemon", "noreply", "no-reply"]) {
            if header :matches "Subject" "*" {
                set "subject" "${1}";
            }
            vacation :days 3
                     :from "Jane Doe <jane@example.org>"
                     :addresses ["jane@example.org", "j.doe@example.org"]
                     :subject "Out of office: ${subject}"
                     :handle "summer-vacation"
                     text:
Hello,

I am on vacation and will read your message about "${subject}" when
I am back. For urgent matters please contact support@example.org.

Best regards,
Jane
.
;
        }
    }
}
