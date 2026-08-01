/// <reference types="../../types/chaqmoq.d.ts" />
//
// Deliberately broken. Used by docs/tools/capture.sh to screenshot what a
// failure looks like: stack frames point at this TypeScript source, not at the
// JavaScript the transpiler produced.

interface Order {
  id: number;
  total: number;
}

function findOrder(orders: Order[], id: number): Order {
  const order = orders.find((candidate) => candidate.id === id);
  if (!order) {
    throw new Error(`order ${id} not found`);
  }
  return order;
}

function chargeCustomer(orders: Order[], id: number): number {
  return findOrder(orders, id).total;
}

const orders: Order[] = [{ id: 1, total: 250 }];
console.log("charging order 1:", chargeCustomer(orders, 1));
console.log("charging order 7:", chargeCustomer(orders, 7));
