# 1. Order events go through a managed broker

## Context

Fulfilment, invoicing, and analytics each need every placed order as it
happens. The storefront places orders over HTTP and must not wait on any of
them, so the orders service hands each order on and answers at once.

## Decision

Publish each order event to Kafka on Confluent Cloud: one topic for order
events, each message keyed by the order id.

## Consequences

An order with no lines, or with a line whose quantity is below one, is
refused before any event is published.

A placed order is acknowledged once its event is published, and the
storefront receives the order id.
