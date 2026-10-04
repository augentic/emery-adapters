# Sharing a wishlist

Two stories: sharing a wishlist by link, and taking the share back.

## Share a wishlist by link

As a member, I want to share my wishlist with friends and family by a link,
so that they can see what I would like without my sending them a list.

Acceptance:

- Sharing creates one link per wishlist; sharing again returns the same
  link until it is revoked.
- Anyone holding the link sees the items, their prices, and which items have
  already been bought for the member, without signing in.
- The link never reveals the member's email address or delivery address.
- Items the member marks as private are left out of the shared view.
- A visitor who buys an item from the shared view marks it as bought for
  the member; the member sees only that it was bought, not by whom.

## Revoke a shared link

As a member, I want to revoke a shared link, so that a link I regret
sharing stops working.

Acceptance:

- Revoking takes effect immediately; a visitor opening the old link sees
  a message that the wishlist is no longer shared.
- Sharing again after a revocation creates a new link; the revoked link
  never works again.
- Purchases recorded through the old link are kept.
