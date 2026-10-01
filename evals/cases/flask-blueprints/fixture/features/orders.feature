Feature: Orders
  Customers place and cancel orders over the JSON API.

  Scenario: An order is placed
    Given a signed-in customer
    When they place an order with one line of 2 at 1200 cents
    Then the order is pending with a total of 3250 cents

  Scenario: A paid order is cancelled
    Given a paid order
    When the customer cancels it
    Then the order is cancelled

  Scenario: A shipped order cannot be cancelled
    Given a shipped order
    When the customer cancels it
    Then the answer is 409 not_cancellable
