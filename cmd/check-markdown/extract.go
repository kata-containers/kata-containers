//
// Copyright (c) 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

package main

import (
	bf "gopkg.in/russross/blackfriday.v2"
)

// linkDescription extracts the description from the specified link node.
func linkDescription(l *bf.Node) (string, error) {
	if err := checkNode(l, bf.Link); err != nil {
		return "", err
	}

	// A link description can be comprised of various elements so scan
	// through them to build up the final value.

	text := ""
	node := l.FirstChild

	for node != nil {
		switch node.Type {
		case bf.Code:
			text += string(node.Literal)
		case bf.Text:
			text += string(node.Literal)
		default:
			logger.WithField("node", node).Debug("ignoring node")
		}

		if node == l.LastChild {
			break
		}

		node = node.Next
	}

	return text, nil
}

// headingName extracts the heading name from the specified Heading node.
func headingName(h *bf.Node) (string, error) {
	if err := checkNode(h, bf.Heading); err != nil {
		return "", err
	}

	// A heading can be comprised of various elements so scan
	// through them to build up the final value.

	text := ""
	node := h.FirstChild

	for node != nil {
		switch node.Type {
		case bf.Code:
			text += string(node.Literal)
		case bf.Text:
			text += string(node.Literal)
		case bf.Link:
			// yep, people do crazy things like adding links into titles!
			descr, err := linkDescription(node)
			if err != nil {
				return "", err
			}

			text += descr
		default:
			logger.WithField("node", node).Debug("ignoring node")
		}

		if node == h.LastChild {
			break
		}

		node = node.Next
	}

	return text, nil
}
