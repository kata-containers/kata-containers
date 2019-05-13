// Package pkgcloud allows you to talk to the packagecloud API.
// See https://packagecloud.io/docs/api
package pkgcloud

import (
	"encoding/json"
	"errors"
	"fmt"
	"net/http"
	"os"
	"regexp"
	"strconv"

	"github.com/mlafeldt/pkgcloud/upload"
	"github.com/peterhellberg/link"
	pb "gopkg.in/cheggaaa/pb.v1"
)

//go:generate bash -c "./gendistros.py supportedDistros | gofmt > distros.go"

// ServiceURL is the URL of packagecloud's API.
const ServiceURL = "https://packagecloud.io/api/v1"

const UserAgent = "pkgcloud Go client"

// A Client is a packagecloud client.
type Client struct {
	token       string
	progressBar bool
}

// NewClient creates a packagecloud client. API requests are authenticated
// using an API token. If no token is passed, it will be read from the
// PACKAGECLOUD_TOKEN environment variable.
func NewClient(token string) (*Client, error) {
	if token == "" {
		token = os.Getenv("PACKAGECLOUD_TOKEN")
		if token == "" {
			return nil, errors.New("PACKAGECLOUD_TOKEN unset")
		}
	}
	return &Client{token, false}, nil
}

// Print a progress bar of paginated API requests when show is set to true
func (c *Client) ShowProgress(show bool) {
	c.progressBar = show
}

// decodeResponse checks http status code and tries to decode json body
func decodeResponse(resp *http.Response, respJson interface{}) error {
	switch resp.StatusCode {
	case http.StatusOK, http.StatusCreated:
		return json.NewDecoder(resp.Body).Decode(respJson)
	case http.StatusUnauthorized, http.StatusNotFound:
		return fmt.Errorf("HTTP status: %s", http.StatusText(resp.StatusCode))
	case 422: // Unprocessable Entity
		var v map[string][]string
		if err := json.NewDecoder(resp.Body).Decode(&v); err != nil {
			return err
		}
		for _, messages := range v {
			for _, msg := range messages {
				// Only return the very first error message
				return errors.New(msg)
			}
			break
		}
		return fmt.Errorf("invalid HTTP body")
	default:
		return fmt.Errorf("unexpected HTTP status: %d", resp.StatusCode)
	}
}

// CreatePackage pushes a new package to packagecloud.
func (c Client) CreatePackage(repo, distro, pkgFile string) error {
	var extraParams map[string]string
	if distro != "" {
		distID, ok := supportedDistros[distro]
		if !ok {
			return fmt.Errorf("invalid distro name: %s", distro)
		}
		extraParams = map[string]string{
			"package[distro_version_id]": strconv.Itoa(distID),
		}
	}

	endpoint := fmt.Sprintf("%s/repos/%s/packages.json", ServiceURL, repo)
	request, err := upload.NewRequest(endpoint, extraParams, "package[package_file]", pkgFile)
	if err != nil {
		return err
	}
	request.SetBasicAuth(c.token, "")
	request.Header.Add("User-Agent", UserAgent)

	client := &http.Client{}
	resp, err := client.Do(request)
	if err != nil {
		return err
	}
	defer resp.Body.Close()

	return decodeResponse(resp, &struct{}{})
}

type Package struct {
	Name           string `json:"name"`
	Filename       string `json:"filename"`
	DistroVersion  string `json:"distro_version"`
	Version        string `json:"version"`
	Release        string `json:"release"`
	Type           string `json:"type"`
	PackageUrl     string `json:"package_url"`
	PackageHtmlUrl string `json:"package_html_url"`
}

// All list all packages in repository
func (c Client) All(repo string) ([]Package, error) {
	endpoint := fmt.Sprintf("%s/repos/%s/packages.json", ServiceURL, repo)
	return c.paginatedRequest(http.MethodGet, endpoint)
}

// Destroy removes package from repository.
//
// repo should be full path to repository
// (e.g. youruser/repository/ubuntu/xenial).
func (c Client) Destroy(repo, packageFilename string) error {
	endpoint := fmt.Sprintf("%s/repos/%s/%s", ServiceURL, repo, packageFilename)
	_, err := c.apiRequest(http.MethodDelete, endpoint, &struct{}{})

	return err
}

// Search searches packages from repository.
// repo should be full path to repository
// (e.g. youruser/repository/ubuntu/xenial).
// q: The query string to search for package filename. If empty string is passed, all packages are returned
// filter: Search by package type (RPMs, Debs, DSCs, Gem, Python) - Ignored when dist != ""
// dist: The name of the distribution the package is in. (i.e. ubuntu, el/6) - Overrides filter.
// perPage: The number of packages to return from the results set. If nothing passed, default is 30
func (c Client) Search(repo, q, filter, dist string, perPage int) ([]Package, error) {
	endpoint := fmt.Sprintf("%s/repos/%s/search.json?q=%s&filter=%s&dist=%s&per_page=%d", ServiceURL, repo, q, filter, dist, perPage)
	return c.paginatedRequest(http.MethodGet, endpoint)
}


func (c Client) paginatedRequest(method string, endpoint string) ([]Package, error) {
	var (
		allPackages []Package
		getLastPage bool
	)

	newPage := make(chan bool)
	total := make(chan int)

	if c.progressBar {
		getLastPage = true
		progBar := pb.New(0)
		progBar.SetMaxWidth(100)
		progBar.Start()
		defer func() {
			progBar.Increment()
			progBar.Finish()
		}()

		go func() {
			for {
				select {
				case np := <-newPage:
					if !np {
						return
					}
					progBar.Increment()
				case t := <-total:
					progBar.SetTotal(t)
				}
			}
		}()
	}

	for {
		var pkgs []Package
		var group link.Group
		group, err := c.apiRequest(method, endpoint, &pkgs)
		if err != nil {
			return nil, err
		}

		allPackages = append(allPackages, pkgs...)

		next, found := group["next"]
		newPage <- found
		if !found {
			break
		}
		endpoint = next.URI

		if !getLastPage {
			continue
		}

		if last, found := group["last"]; found {
			re := regexp.MustCompile(`page=(\d+)$`)
			pages, err := strconv.Atoi(re.FindStringSubmatch(last.URI)[1])
			if err != nil {
				continue
			}
			getLastPage = false
			total <- pages
		}
	}

	return allPackages, nil

}

func (c Client) apiRequest(method string, endpoint string, decodedResp interface{}) (link.Group, error) {
	req, err := http.NewRequest(method, endpoint, nil)
	if err != nil {
		return nil, err
	}

	req.SetBasicAuth(c.token, "")
	req.Header.Add("User-Agent", UserAgent)

	client := &http.Client{}
	resp, err := client.Do(req)
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()

	err = decodeResponse(resp, decodedResp)
	if err != nil {
		return nil, err
	}

	// API are paginated, with next page in the response header, as "Link"
	// element
	return link.ParseResponse(resp), nil
}
