//
// Copyright (c) 2017 Intel Corporation
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//

package hyperstart

import (
	"encoding/json"
	"fmt"
	"net"
	"sync"
)

type ctlDataType string

const (
	eventType ctlDataType = "ctlEvent"
	replyType ctlDataType = "ctlReply"
)

type multicast struct {
	bufReplies []*DecodedMessage
	reply      []chan *DecodedMessage
	event      map[string]chan *DecodedMessage
	ctl        net.Conn
	sync.Mutex
}

func newMulticast(ctlConn net.Conn) *multicast {
	return &multicast{
		bufReplies: []*DecodedMessage{},
		reply:      []chan *DecodedMessage{},
		event:      make(map[string]chan *DecodedMessage),
		ctl:        ctlConn,
	}
}

func startCtlMonitor(ctlConn net.Conn, done chan<- interface{}) *multicast {
	ctlMulticast := newMulticast(ctlConn)

	go func() {
		for {
			msg, err := ReadCtlMessage(ctlMulticast.ctl)
			if err != nil {
				hyperLog.Infof("Read on CTL channel ended: %s", err)
				break
			}

			err = ctlMulticast.write(msg)
			if err != nil {
				hyperLog.Errorf("Multicaster write error: %s", err)
				break
			}
		}

		close(done)
	}()

	return ctlMulticast
}

func (m *multicast) buildEventID(containerID, processID string) string {
	return fmt.Sprintf("%s-%s", containerID, processID)
}

func (m *multicast) sendEvent(msg *DecodedMessage) error {
	var paeData PAECommand

	err := json.Unmarshal(msg.Message, paeData)
	if err != nil {
		return err
	}

	uniqueID := m.buildEventID(paeData.Container, paeData.Process)
	channel, exist := m.event[uniqueID]
	if !exist {
		return nil
	}

	channel <- msg

	delete(m.event, uniqueID)

	return nil
}

func (m *multicast) sendReply(msg *DecodedMessage) error {
	m.Lock()
	if len(m.reply) == 0 {
		m.bufReplies = append(m.bufReplies, msg)
		m.Unlock()
		return nil
	}

	replyChannel := m.reply[0]
	m.reply = m.reply[1:]

	m.Unlock()

	// The current reply channel has been removed from the list, that's why
	// we can be out of the mutex to send through that channel. Indeed, there
	// is no risk that someone else tries to write on this channel.
	replyChannel <- msg

	return nil
}

func (m *multicast) processBufferedReply(channel chan *DecodedMessage) {
	m.Lock()

	if len(m.bufReplies) == 0 {
		m.reply = append(m.reply, channel)
		m.Unlock()
		return
	}

	msg := m.bufReplies[0]
	m.bufReplies = m.bufReplies[1:]

	m.Unlock()

	// The current buffered reply message has been removed from the list, and
	// the channel have not been added to the reply list, that's why we can be
	// out of the mutex to send the buffered message through that channel.
	// There is no risk that someone else tries to write this message on another
	// channel, or another message on this channel.
	channel <- msg
}

func (m *multicast) write(msg *DecodedMessage) error {
	switch msg.Code {
	case NextCode:
		return nil
	case ProcessAsyncEventCode:
		return m.sendEvent(msg)
	default:
		return m.sendReply(msg)
	}
}

func (m *multicast) listen(containerID, processID string, dataType ctlDataType) (chan *DecodedMessage, error) {
	switch dataType {
	case replyType:
		newChan := make(chan *DecodedMessage)

		go m.processBufferedReply(newChan)

		return newChan, nil
	case eventType:
		uniqueID := m.buildEventID(containerID, processID)

		_, exist := m.event[uniqueID]
		if exist {
			return nil, fmt.Errorf("Channel already assigned for ID %s", uniqueID)
		}

		m.event[uniqueID] = make(chan *DecodedMessage)

		return m.event[uniqueID], nil
	default:
		return nil, fmt.Errorf("Unknown data type: %s", dataType)
	}
}
