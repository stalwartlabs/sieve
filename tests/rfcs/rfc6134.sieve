require ["envelope", "extlists", "fileinto", "spamtest", "foreverypart", "mime", "enclose",
    "relational", "comparator-i;ascii-numeric", "variables", "date", "enotify", "subaddress", "index", "reject"];
if envelope :list "from" ":addrbook:default"
{ /* Known: allow high spam score */
if spamtest :value "ge" :comparator "i;ascii-numeric" "8"
    {
    fileinto "spam";
    }
}
elsif spamtest :value "ge" :comparator "i;ascii-numeric" "3"
{ /* Unknown: less tolerance in spam score */
fileinto "spam";
}

if valid_ext_list "addrbook" {
    keep;
}

if envelope :list "from" ":addrbook:default" {
set "lim" "8";  /* Known: allow high spam score */
} else {
set "lim" "3";  /* Unknown: less tolerance in spam score */
}
if spamtest :value "ge" :comparator "i;ascii-numeric" "${lim}" {
fileinto "spam";
}

if currentdate :list "date"
"tag:example.com,2011-01-01:localHolidays" {
notify "xmpp:romeo@im.example.com";
}

if allof (envelope :detail "to" "mylist",
        header :list "from"
            "tag:example.com,2010-05-28:mylist") {
redirect :list "tag:example.com,2010-05-28:mylist";
}

if header :index 1 :matches "received" "*(* [*.*.*.*])*" {
set "ip" "${3}.${4}.${5}.${6}";
if string :list "${ip}"
    "tag:example.com,2011-04-10:DisallowedIPs" {
reject "Message not allowed from this IP address";
}
}

foreverypart
{
if header :mime :param "filename"
:list ["Content-Type", "Content-Disposition"]
    "tag:example.com,2011-04-10:BadFileNameExts"
{
# these attachment types are executable
enclose :subject "Warning" text:
WARNING!
.
;
break;
}
}