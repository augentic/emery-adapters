# Accounts

Depends on: [membership eligibility](../policies/membership-eligibility.md),
[notifications](notifications.md).

An account is the record the service keeps of one person: their contact
details, their membership, their loans, reservations, and charges, and
their sign-in credentials. This document covers three things an account
does: membership, signing in, and resetting a forgotten password.

## Membership

An account is created with a name, a postal address, and at least one of
an email address and a mobile number; an account with neither contact
channel is refused. Creating the account also creates a membership
application, which a volunteer approves or declines at the desk after
checking proof of address against the eligibility policy. An application
that has not been approved or declined within thirty days lapses, and the
applicant is told; they may apply again.

A membership has one of four states: `applied`, `active`, `lapsed`, and
`suspended`. Approval moves an application to `active` and sets the
anniversary to one year from the day of approval. An `active` membership
becomes `lapsed` on the day after its anniversary unless it has been
renewed; renewal before the anniversary extends the anniversary by one
year, and renewal after it moves `lapsed` back to `active` with a new
anniversary a year from the renewal. A committee member may move an
`active` membership to `suspended` with a reason and an end date no more
than six months away, and `suspended` returns to `active` on the end date
or when a committee member lifts it. Only an `active` membership may
borrow or reserve; every other state may browse the catalogue and see its
own account, and the service says which state blocks an action rather
than refusing silently.

Each account has at most one membership card, identified by its number.
A card reported lost is cancelled at once and a replacement issued with a
new number; scanning a cancelled card at the desk is refused and the
volunteer is shown the replacement number. A member's loan history,
reservations, and charges stay with the account across card changes and
across lapses and renewals, and are kept for six years after the account
is closed, as the committee's record-keeping requires.

## Signing in

A member signs in with their email address or mobile number and a
password, or with a six-digit code sent to the mobile number on the
account. A password is at least twelve characters and is checked against
a list of commonly used passwords when it is set; a password on the list
is refused with a message saying so. Five failed sign-ins within fifteen
minutes lock the account for fifteen minutes, and the member is told the
lock will lift by itself; a sign-in during the lock is refused without
extending it. A one-time code is valid for ten minutes and for one use,
and three wrong codes invalidate it.

A signed-in session lasts thirty days in the app and until the browser is
closed on the web, and the member can end every session from the account
page. Changing the password ends every other session. The volunteer's desk
view signs in a volunteer, not a member: a volunteer acting for a member at
the desk does so from the volunteer's own session and the action is
recorded against both.

## Resetting a forgotten password

A member who has forgotten their password asks for a reset from the
sign-in screen with the email address or mobile number on the account. If
the address or number is on an account, a reset link is sent to it; if it
is not, nothing is sent — and in both cases the screen says the same thing,
that a link has been sent if the details match an account, so that the
screen cannot be used to find out which addresses have accounts.

The link is valid for thirty minutes and for one use, and asking again
before it is used invalidates the earlier link. Following the link lets the
member set a new password under the same rules as any password, and
setting it ends every existing session on the account. A member who cannot
receive the link — the address is old, the phone is lost — asks a volunteer
at the desk, who checks their identity against the proof of address on
file and sets a temporary password that must be changed at the next sign
in.
