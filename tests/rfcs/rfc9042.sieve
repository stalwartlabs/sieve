require ["fileinto", "mailboxid", "mailbox", "enotify", "fcc"];

if header :contains ["from"] "coyote" {
    fileinto :mailboxid "F6352ae03-b7f5-463c-896f-d8b48ee3"
             "INBOX.harassment";
}

fileinto :mailboxid "Fnosuch"
         :create
         "INBOX.no-such-folder";
            # creates INBOX.no-such-folder, but it doesn't
            # get the "Fnosuch" mailboxid.

notify :fcc "INBOX.Sent"
       :mailboxid "F6352ae03-b7f5-463c-896f-d8b48ee3"
       :message "You got mail!"
       "mailto:ken@example.com";

if header :contains ["from"] "coyote" {
    if mailboxidexists "F6352ae03-b7f5-463c-896f-d8b48ee3" {
        fileinto :mailboxid "F6352ae03-b7f5-463c-896f-d8b48ee3"
                            "INBOX.name.will.not.be.used";
    } else {
        fileinto "INBOX.harassment";
    }
}
