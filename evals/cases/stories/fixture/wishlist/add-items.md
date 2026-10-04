# Adding items to a wishlist

## Add an item from a product page

As a member, I want to add a product to my wishlist from its product page,
so that I can come back to it later without searching again.

Acceptance:

- The product page shows an "Add to wishlist" control to a signed-in member
  and a sign-in prompt to a visitor.
- Adding a product already on the wishlist does not create a second entry;
  the existing entry is kept and the member is told it is already there.
- A wishlist holds at most 200 items; adding to a full wishlist is refused
  with a message naming the limit.
- A product that is no longer sold stays on the wishlist, marked as
  unavailable, until the member removes it.
- The new item appears at the top of the wishlist.

Notes: the wishlist is one list per member for this release; named lists
are a later story.
