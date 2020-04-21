package mutating

import (
	"context"
	"fmt"

	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"

	"github.com/slok/kubewebhook/pkg/log"
)

// Mutator knows how to mutate the received kubernetes object.
type Mutator interface {
	// Mutate will received a pointr to an object that can be mutated
	// mutators are grouped in chains so the mutate method can return
	// a stop boolean to stop executing the chain and also an error.
	Mutate(context.Context, metav1.Object) (stop bool, err error)
}

// MutatorFunc is a helper type to create mutators from functions.
type MutatorFunc func(context.Context, metav1.Object) (bool, error)

// Mutate satisfies Mutator interface.
func (f MutatorFunc) Mutate(ctx context.Context, obj metav1.Object) (bool, error) {
	return f(ctx, obj)
}

// Chain is a chain of mutators that will execute secuentially all the
// mutators that have been added to it. It satisfies Mutator interface.
type Chain struct {
	mutators []Mutator
	logger   log.Logger
}

// NewChain returns a new chain.
func NewChain(logger log.Logger, mutators ...Mutator) *Chain {
	return &Chain{
		mutators: mutators,
		logger:   logger,
	}
}

// Mutate will execute all the mutation chain.
func (c *Chain) Mutate(ctx context.Context, obj metav1.Object) (bool, error) {
	for _, mt := range c.mutators {
		select {
		case <-ctx.Done():
			return false, fmt.Errorf("mutator chain not finished correctly, context ended")
		default:
			stop, err := mt.Mutate(ctx, obj)
			if stop || err != nil {
				return true, err
			}
		}
	}

	// Return false if used a chain of chains.
	return false, nil
}
