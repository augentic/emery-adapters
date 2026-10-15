@publish
Feature: Convert Received Train messages to Vehicle events

  Scenario: Publish Vehicle Event for station "0"
    Given vehicles for the trip "AMP123"
    And filter for stations "0,40,19"
    When an arrival event for "0" is received with "0" seconds delay
    Then 2 arrival vehicle events is created

  Scenario: Publish Vehicle Event for station "0"
    Given vehicles for the trip "AMP123"
    And filter for stations "0,40,19"
    When an departure event for "0" is received with "0" seconds delay
    Then 2 departure vehicle events is created

  Scenario: Publish Vehicle Event for station "40"
    Given vehicles for the trip "AMP123"
    And filter for stations "0,40,19"
    When an arrival event for "40" is received with "0" seconds delay
    Then 2 arrival vehicle events is created

  Scenario: Publish Vehicle Event for station "40"
    Given vehicles for the trip "AMP123"
    And filter for stations "0,40,19"
    When an departure event for "40" is received with "0" seconds delay
    Then 2 departure vehicle events is created

  Scenario: No event should be published for not filetered station
    Given vehicles for the trip "AMP123"
    And filter for stations "0,19"
    When an arrival event for "40" is received with "0" seconds delay
    Then no event should be generated

  Scenario: No event should be published if no vehicles provided
    Given filter for stations "0,40,19"
    When an arrival event for "40" is received with "0" seconds delay
    Then no event should be generated

  Scenario: No event should be published if no static stop info available
    Given vehicles for the trip "AMP123"
    And filter for stations "0,40,19,80"
    When an arrival event for "80" is received with "0" seconds delay
    Then no event should be generated
  
  Scenario: No event should be published if no trainUpdate
    Given vehicles for the trip "AMP123"
    And filter for stations "0,40,19"
    When an event without trainUpdate
    Then no event should be generated

  Scenario: No event should be published if no changes
    Given vehicles for the trip "AMP123"
    And filter for stations "0,40,19"
    When an event without changes
    Then no event should be generated

  Scenario: No event should be published if no actual changes
    Given vehicles for the trip "AMP123"
    And filter for stations "0,40,19"
    When an event without actual changes
    Then no event should be generated
  
  Scenario: No event should be published if more than 60 seconds later than current time
    Given vehicles for the trip "AMP123"
    And filter for stations "0,40,19"
    When an departure event for "0" is received with "61" seconds delay
    Then no event should be generated

  Scenario: No event should be published if more than 30 seconds earlier than current time
    Given vehicles for the trip "AMP123"
    And filter for stations "0,40,19"
    When an departure event for "0" is received with "-31" seconds delay
    Then no event should be generated