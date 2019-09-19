import React from 'react';
import { Link } from 'react-router-dom';
import { withRouter } from 'react-router-dom';
import createBrowserHistory from 'history/createBrowserHistory';

import Paper from '@material-ui/core/Paper';
import Tabs from '@material-ui/core/Tabs';
import Tab from '@material-ui/core/Tab';

const history = createBrowserHistory();

function NavTabs(comp) {
    return (
        <Paper square>
            <Tabs
                value={history.location.pathname}
                indicatorColor="primary"
                textColor="primary"
                onChange={(event, v) => { history.push(v); comp.props.remountAction() } }
                aria-label="Tourcalc nav"
            >
                <Tab label="Choose" value="/choose"
                    component={Link} to="/choose"
                />
                <Tab label="Persons" value="/persons"
                    component={Link} to="/persons"
                >xxx</Tab>
                <Tab label="Spendings" value="/spendings"
                    component={Link} to="/spendings"
                />
                <Tab label={comp.props.tour.name} value="/auth"
                    component={Link} to="/auth"
                />
            </Tabs>
        </Paper>
    );
}

export default class Header extends React.Component {
    constructor(props) {
        super(props);
        this.state = {
            tab: 0
        };
    }
    render() {
        return NavTabs(this);
        {/*return (
            <header>
                <menu>
                    <ul>
                        <li>
                            <Link to="/persons">Tour Persons</Link>
                        </li>
                        <li>
                            <Link to="/spendings">Tour Spendings</Link>
                        </li>
                        <li>
                            <Link to="/">Choose Tour</Link>
                        </li>
                        <li>
                            <Link to="/auth">Auth</Link>
                        </li>
                        <li> Chosen tour: {this.props.tour == null ? "n/a" : this.props.tour.name} </li>
                    </ul>
                </menu>
            </header>
        );*/}
    }
};