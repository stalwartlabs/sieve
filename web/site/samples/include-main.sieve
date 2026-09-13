# Includes and global variables
#
# The "common" tab is an included script. It sets global variables that
# this script reads back. Add your own tabs with the + button: a tab named
# "foo" is available as include "foo".

require ["include", "variables", "fileinto", "mailbox"];

global ["priority", "project"];

include :personal "common";

if string :is "${priority}" "high" {
    fileinto :create "Projects/${project}/Priority";
} else {
    fileinto :create "Projects/${project}";
}
