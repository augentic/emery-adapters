# Toolshed documentation

Toolshed is the lending library for tools run by the Hollow Lane residents'
association: a catalogue of around four hundred hand and power tools kept
in the old scout hut, lent to members for a week at a time, reserved ahead
when a tool is out, and returned to a volunteer at the desk. This tree is
the written description of how it works, for members, for the volunteers
who staff the desk, and for the people who maintain the software behind
it.

The tree has three parts.

- [guides/](guides/) walks a member or a volunteer through each thing they
  do: joining, borrowing, reserving, returning, and reporting damage.
- [reference/](reference/) states precisely how each part of the service
  behaves — accounts, the catalogue, loans, reservations, returns, and the
  notifications the service sends — together with the error codes, a
  glossary, and the changelog.
- [policies/](policies/) records the rules the committee has set: who may
  join, what happens when a tool comes back late, how damage and loss are
  charged, how much one member may borrow, and how far ahead a tool may be
  reserved.

Where a guide and a policy describe the same thing, the policy is the
rule and the guide is the explanation; where the reference and either of
them differ, the reference is wrong and should be corrected. Each
document names the others it depends on at the top.

The desk is open on Tuesday and Thursday evenings from six until eight and
on Saturday mornings from nine until twelve. Everything a member does in
the app can be done at the desk with a volunteer's help, and some things —
returning a tool, paying a charge in cash — can only be done there.

## Reading this tree

A member new to Toolshed reads the guides in order and the policies on
late returns and damage before their first loan. A volunteer reads all of
the guides and the references for loans, reservations, and returns, which
describe what the desk view shows them and what each scan does. A
committee member reads the policies, which are theirs to change, and the
changelog, which records when each rule changed and why.

The people who maintain the software read the references first and the
policies second: the references say what the service does, down to the
state names it uses and the order it checks things in, and the policies
say why and give the numbers the references quote. The glossary fixes the
words, and the error codes table ties every refusal the service makes to
the rule behind it.

## Changing these documents

A change to a rule is a committee decision first and an edit second, and
the edit is recorded in the changelog with the date it took effect at the
desk. A change to the explanation in a guide, or a correction to a
reference that had drifted from the policy it describes, needs no decision
and is made by whoever notices, with a changelog line all the same. The
documents are kept in the same repository as the software, so that a
release of the service and the documents describing it move together.
