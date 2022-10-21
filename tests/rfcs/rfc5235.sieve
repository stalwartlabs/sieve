require ["spamtestplus", "fileinto", "relational", "comparator-i;ascii-numeric", "virustest"];

if spamtest :value "eq" :comparator "i;ascii-numeric" "0"
{
    fileinto "INBOX.unclassified";
}
elsif spamtest :value "ge" :comparator "i;ascii-numeric" "3"
{
    fileinto "INBOX.spam-trap";
}

if spamtest :value "eq"
            :comparator "i;ascii-numeric" "0"
{
    fileinto "INBOX.unclassified";
}
elsif spamtest :percent :value "eq"
                :comparator "i;ascii-numeric" "0"
{
    fileinto "INBOX.not-spam";
}
elsif spamtest :percent :value "lt"
                :comparator "i;ascii-numeric" "37"
{
    fileinto "INBOX.spam-trap";
}
else
{
    discard;
}

if spamtest :percent :count "eq"
            :comparator "i;ascii-numeric" "0"
{
    fileinto "INBOX.unclassified";
}
elsif spamtest :percent :value "eq"
                :comparator "i;ascii-numeric" "0"
{
    fileinto "INBOX.not-spam";
}
elsif spamtest :percent :value "lt"
                :comparator "i;ascii-numeric" "37"
{
    fileinto "INBOX.spam-trap";
}
else
{
    discard;
}

if virustest :value "eq" :comparator "i;ascii-numeric" "0"
{
    fileinto "INBOX.unclassified";
}
if virustest :value "eq" :comparator "i;ascii-numeric" "4"
{
    fileinto "INBOX.quarantine";
}
elsif virustest :value "eq" :comparator "i;ascii-numeric" "5"
{
    discard;
}

