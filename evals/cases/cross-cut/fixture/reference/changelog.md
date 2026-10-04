# Changelog

What changed in the service and in these documents, newest first. A
change to a rule names the policy or reference it changed; a change to the
app alone is marked as such. Dates are the date the change took effect at
the desk.

## 2026-03-14

- Late returns policy: the first day late is free. Previously the late
  charge accrued from the first day; the committee agreed at the February
  meeting that a tool back the day after it was due should cost nothing.
- Returns reference and returning guide updated to match.
- App: the receipt sent at return now shows the condition recorded and
  the charge arising, not just the time.

## 2026-01-20

- Reservation limits policy: a member may hold three open reservations,
  up from two, and the reduced limit after missed collections is one.
- Reservations reference: a shelf hold now lasts until the end of the next
  desk session, not the end of the day, since the Saturday session made
  "end of day" ambiguous for a hold made on Friday.
- App: the account page shows the estimated turn for each reservation.

## 2025-11-08

- Fair use policy: high-demand tools (three-day loans) count double toward
  the loan limit, and only one may be out at a time per member.
- Catalogue reference: a tool's loan period is part of its entry and shown
  on its page.
- Error codes: `LOAN-014` and `LOAN-015` added.

## 2025-09-30

- Damage and loss policy: reported and unreported damage are charged at
  different rates, and a report stops the late clock. Before this change
  all damage was charged the same and members had no reason to report
  early.
- Reporting damage guide written.
- Error codes: `DMG-001` to `DMG-004` added; `CHG-004` retired, since the
  per-loan charge cap it refused under was removed.

## 2025-07-12

- Accounts reference: sign-in by one-time code to the mobile number on the
  account, in addition to password. Five failed sign-ins lock the account
  for fifteen minutes.
- Accounts reference: a password reset link is valid for thirty minutes
  and one use, and the reset screen gives the same answer whether or not
  the details match an account.
- Notifications reference: the "password changed" and "sign-in locked"
  notifications added.

## 2025-05-03

- Membership eligibility policy: proof of address must be dated within
  three months; previously six. Lapsed members keep their loan history.
- Accounts reference: an application undecided for thirty days lapses.
- Notifications reference: written. The quiet hours (eight in the morning
  to nine at night) and the one retry after an hour date from here.

## 2025-02-15

- Returns reference: a loan twenty-eight days overdue is written off as
  lost and charged at replacement value, with the late charge waived; a
  return within a further twenty-eight days refunds the replacement charge
  and reinstates the late charge. Previously overdue loans ran on
  indefinitely and the charges were never collected.
- Loans reference: the unpaid-charge threshold that blocks a new loan is
  ten pounds.

## 2024-12-06

- App: catalogue pages made public, so a member can send a link to a tool
  to someone who is not signed in. Reserving from a page still needs an
  active membership.
- Catalogue reference: donations enter the catalogue as proposed entries
  and are inspected before acceptance; petrol tools and chainsaws are
  refused at the point of proposal, as the insurer requires.
- Error codes: `CAT-005` added.

## 2024-11-15

- App: the volunteer's desk view shows, when a tool is scanned for return,
  the case contents recorded when the loan opened, so a shortfall can be
  attributed to the right loan. Before this the desk view showed only the
  catalogue's list and every shortfall fell on the member returning.
- Loans reference: case contents are checked and any shortfall recorded
  against the previous loan before a new loan opens.

## 2024-11-01

- First release of the service and of this documentation. Membership,
  the catalogue, loans with one extension, reservations with a queue, and
  returns at the desk. Policies: membership eligibility, late returns,
  damage and loss, fair use.
- The paper ledger kept in the hut since 2019 was closed; its loans were
  entered into the service as history, with the condition findings left
  blank where the ledger recorded none.

## Before the service

The paper ledger ran from the first desk session in March 2019 to the end
of October 2024. It recorded the tool, the member's name, the date out,
and the date back, in a volunteer's handwriting, and nothing else; late
returns were chased by whoever noticed, and charges were a matter of
conscience and a jar on the desk. The committee's decision to replace it,
minuted in May 2024, cited three lost tools nobody could trace, a queue
for the floor sander kept on a sticky note, and a volunteer who had
written the same member's name four different ways.
