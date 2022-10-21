require ["copy", "redirect-deliverby", "date", "variables",
        "relational", "comparator-i;ascii-numeric", "envelope", "envelope-dsn",
        "envelope-deliverby", "redirect-dsn", "fileinto"];

# Check whether SUCCESS notifications were requested,
# irrespective of any other requests that were made
if envelope "notify" "SUCCESS"
{
    # do whatever
}

# Check whether only FAILURE notifications were requested
if allof ( envelope "notify" "FAILURE",
            envelope :comparator "i;ascii-numeric"
                    :count "eq" "notify" "1"
        )
{
    # do whatever
}

# See if the orcpt is an RFC822 address in the example.com
# domain
if envelope :matches "orcpt" "rfc822;*@example.com"
{
    # do whatever
}

# Check to see if this message didn't make it in the time allotted by
# the originator.
if anyof (envelope :contains "bytimerelative" "-",
            envelope :value "eq" :comparator "i;ascii-numeric"
                    "bytimerelative" "0")
{
    # do whatever
}

# Check to see if this message didn't make it in the time allotted by
# the originator.
if currentdate :matches "iso8601" "*" {
    set "cdate" "${0}";
    if envelope :value "ge" "bytimeabsolute" "${cdate}" {
        # do whatever
    }
}

# If the message didn't make it in time, file it according to when it
# should have been received
if envelope :matches :zone "+0000" "bytimeabsolute" "*T*:*:*" {
    set "bdate" "${0}";
    set "bhour" "${2}";
    if currentdate :zone "+0000" :value "lt" "iso8601" "${bdate}" {
        fileinto "missed-${bhour}";
    }
}

# Make a private copy of messages from user@example.com
if address "from" "user@example.com"
{
    redirect :copy :notify "NEVER" "elsewhere@example.com";
}

# Send a copy to my cell phone, time out after 10 minutes
if address "from" "user@example.com"
{
    redirect :copy :bytimerelative 600 "cellphone@example.com";
}

# Send a copy to my cell phone to be delivered before 10PM
if currentdate :value "lt"
                :comparator "i;ascii-numeric" "hour" "22"
{
    if currentdate :matches "date" "*" {set "date" "${0}";}
    if currentdate :matches "zone" "*" {set "zone" "${0}";}
    redirect :copy :bytimeabsolute "${date}T20:00:00${zone}"
            :bymode "return" "cellphone@example.com";
}
