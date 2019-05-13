package obsgo

import (
	"encoding/xml"
	"io"
	"io/ioutil"
	"net/http"
	"path"

	"github.com/pkg/errors"
	"github.com/sirupsen/logrus"
)

// Data structure used for XML unmarshaling of the OBS API responses to retrieve
// the list of binaries inside a package.
type PkgBinary struct {
	Filename string `xml:"filename,attr"`
	Size     string `xml:"size,attr"`
	Mtime    string `xml:"mtime,attr"`
}

type binaryList struct {
	XMLName xml.Name    `xml:"binarylist"`
	Bins    []PkgBinary `xml:"binary"`
}

type xmlDirList struct {
	XMLName xml.Name `xml:"directory"`
	Dirs    []struct {
		Name string `xml:"name,attr"`
	} `xml:"entry"`
}

const (
	apiBaseURL = "https://api.opensuse.org"
)

func (proj *Project) obsRequest(resource string) (io.ReadCloser, error) {
	url := apiBaseURL + path.Join("/build", proj.Name, resource)
	logrus.WithFields(logrus.Fields{
		"url": url,
	}).Debug("obsRequest")

	req, err := http.NewRequest(http.MethodGet, url, nil)
	if err != nil {
		return nil, err
	}
	req.SetBasicAuth(proj.User, proj.Password)
	client := &http.Client{}
	resp, err := client.Do(req)
	if err != nil {
		return nil, err
	}

	if resp.StatusCode != 200 {
		return nil, errors.Errorf("Unexpected HTTP status code: %d", resp.StatusCode)
	}

	logrus.Debugf("obsRequest got HTTP response body: %+v", resp.Body)

	return resp.Body, nil
}

func (proj *Project) listDirectories(path string) ([]string, error) {
	resp, err := proj.obsRequest(path)
	if err != nil {
		return nil, err
	}
	defer resp.Close()

	xmlResp, err := ioutil.ReadAll(resp)
	if err != nil {
		return nil, err
	}

	var list xmlDirList
	err = xml.Unmarshal(xmlResp, &list)
	if err != nil {
		return nil, err
	}

	dirs := make([]string, 0, len(list.Dirs))
	for _, d := range list.Dirs {
		dirs = append(dirs, d.Name)
	}
	return dirs, nil
}

func (proj *Project) listBinaries(path string) ([]PkgBinary, error) {
	var binaries []PkgBinary

	resp, err := proj.obsRequest(path)
	if err != nil {
		return binaries, err
	}
	defer resp.Close()

	xmlResp, err := ioutil.ReadAll(resp)
	if err != nil {
		return binaries, err
	}

	var bList binaryList
	if err := xml.Unmarshal(xmlResp, &bList); err != nil {
		return nil, err
	}

	return bList.Bins, nil
}

func (proj *Project) downloadBinary(path string, dest io.Writer) error {
	resp, err := proj.obsRequest(path)
	if err != nil {
		return err
	}
	defer resp.Close()

	_, err = io.Copy(dest, resp)
	if err != nil {
		return err
	}

	return nil
}
