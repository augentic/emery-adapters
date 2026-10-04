# Returns

Depends on: [loans](loans.md), [catalogue](catalogue.md),
[late returns](../policies/late-returns.md),
[damage and loss](../policies/damage-and-loss.md).

A return closes a loan. It happens at the desk, when a volunteer scans a
tool that is on loan; nothing a member does in the app returns a tool, and
a tool is on loan to its member until that scan whatever else has happened
to it.

## Recording a return

Scanning a tool on loan shows the volunteer the loan, its due date, the
condition and case contents recorded when it opened, and whether any
damage has been reported on it since. The volunteer records the condition
they find — `good`, `worn`, or damaged, with a note and a photograph when
damaged — and ticks off the case contents against the catalogue entry. The
return is recorded with the date and time of the scan and the volunteer
who made it, the loan closes, and the member is sent a receipt carrying
the return time, the condition recorded, and any charge arising.

A return is never backdated. A tool found on the shelf or in the hut
without having been scanned in is scanned in when it is found, and the
return time is the time of that scan; the member may dispute the late
charge that results, and the committee decides.

## Lateness

A loan is late when the tool is scanned in after its due date. The number
of days late is the number of whole days from the due date to the return
date: a tool due on the fourth and returned on the fifth is one day late,
and one due on the fourth and returned on the fourth is not late. The
first day late carries no charge; from the second day the late charge in
the late returns policy accrues per day and is added to the account when
the return is recorded. A loan whose tool was reported damaged accrues no
late charge from the day of the report, as the damage and loss policy
says, and a loan on a lapsed or suspended membership counts its lateness
from the day of the lapse or suspension if that is earlier than the due
date.

A loan twenty-eight days past its due date with no return is written off
as lost: the tool's replacement value is charged, the late charge is
waived, and the loan closes. A tool returned within twenty-eight days of
being written off reopens the loan as returned on the day it comes back:
the replacement charge is refunded and the late charge that was waived is
reinstated in its place.

## Condition findings

A condition finding at return that is worse than the condition recorded
at the start of the loan, or a case shortfall not recorded at the start,
is attributed to the loan being returned. If damage was reported on the
loan before the return the finding is reported damage; otherwise it is
unreported damage. The charge for each is the damage and loss policy's and
is added to the account with the return. A member may dispute a finding
within seven days of the receipt; a disputed charge is shown but not
counted toward the unpaid-charge check until the committee has decided.

## After the return

When the returned tool has an open reservation, the head of the queue is
notified at once and the tool becomes `held`; otherwise its availability
becomes `on shelf`, or `unavailable` if the condition finding takes it
out of service. The member's standing — overdue, under limit — is updated
with the return, so a member who returns their one overdue tool may borrow
again in the same session.
