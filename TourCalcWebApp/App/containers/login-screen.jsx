import React from 'react';
import ReactDOM from 'react-dom';
import AppState from './appstate.jsx'
import Cookies from 'js-cookie';
import { BrowserRouter as Router, Route, Switch, Redirect } from 'react-router-dom';


import Table from '@material-ui/core/Table';
import TableBody from '@material-ui/core/TableBody';
import TableCell from '@material-ui/core/TableCell';
import TableHead from '@material-ui/core/TableHead';
import TableRow from '@material-ui/core/TableRow';


export default class LoginScreen extends React.Component {
    constructor(props) {
        super(props);
        this.state = {
            isAuthLoaded: false,
            authData: null
        };
    }
    scope = "code"
    code = "none"
    componentDidMount() {
        document.title = "Touclalc: Login"
    }
    render() {
        if (!this.state.isAuthLoaded) {
            return (

                <form onSubmit={(event) => {
                    event.preventDefault();
                    AppState.login(this, this.scope, this.code)
                }}>
                <Table>
                    <TableHead>
                        <TableRow>
                            <TableCell>
                                Login scope
                            </TableCell>
                            <TableCell>
                                Code
                            </TableCell>
                        </TableRow>
                    </TableHead>

                    <TableBody>
                        <TableRow>
                            <TableCell>
                                <select
                                    defaultValue="code"
                                    onChange={(e) => { this.scope = event.target.value }}
                                >
                                    <option value="code">Code</option>
                                    <option value="admin">Admin</option>
                                </select>
                            </TableCell>
                            <TableCell>
                                <input
                                    type='text'
                                    onChange={(e) => this.code = event.target.value}
                                />
                            </TableCell>
                        </TableRow>
                            <TableRow>
                                <TableCell align="left" colSpan={2}>
                                <input
                                    type='submit'
                                    value="Login"
                                />

                            </TableCell>
                        </TableRow>
                        </TableBody>
                </Table>
                    </form>

            )
        } else {
            return <Redirect to="/"/>
        }
    }
    
};
